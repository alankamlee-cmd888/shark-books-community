#[cfg(test)]
mod tests {
    use super::*;

    fn fact(slot_id: &str, value: &str) -> ActionFact {
        ActionFact::new(slot_id, value, FactSource::Owner).expect("fact")
    }

    fn context_fact(slot_id: &str, value: &str) -> ActionFact {
        ActionFact::new(slot_id, value, FactSource::SystemContext).expect("context fact")
    }

    fn controller(exposed: &[&str]) -> ActionController {
        ActionController::new(load_action_registry().expect("registry"), exposed.iter().copied())
            .expect("controller")
    }

    #[test]
    fn registry_reconciles_authoritative_counts() {
        let registry = load_action_registry().expect("registry");
        assert_eq!(registry.actions.len(), 206);
        assert_eq!(registry.aliases.len(), 5);
        assert_eq!(registry.actions.iter().filter(|a| a.voice_eligible).count(), 175);
        assert_eq!(registry.actions.iter().flat_map(|action| action.utterances.iter()).count(), 700);
        assert_eq!(registry.actions.iter().filter(|a| a.voice_parity == "REQUIRED_WHEN_EXPOSED").count(), 32);
        assert_eq!(registry.actions.iter().filter(|a| a.voice_parity == "REQUIRED_WHEN_ACTIVATED").count(), 143);
        assert!(registry.actions.iter().all(|action| action.required_slots.iter().all(|slot| slot.slot_id != "none")));
    }

    #[test]
    fn batch_b_planning_rows_are_overlaid_with_merged_evidence() {
        let registry = load_action_registry().expect("registry");
        for action_id in ["CONTACTS.LIST", "CONTACTS.SAVE", "SETTINGS.BOOKS_INFO", "SETTINGS.STORAGE_ROOT.SELECT", "REPORT.SUMMARY"] {
            let action = registry.action(action_id).expect("Batch B action");
            assert_eq!(action.implementation_state, "PRODUCTION_MAIN");
            assert_eq!(action.availability_state, "BACKEND_READY_UI_LOCKED");
            assert_eq!(action.evidence_state.as_deref(), Some("SBC7B1_BATCH_B_DUAL_PLATFORM_PASS"));
            assert_eq!(action.backend_state, "READY");
        }
    }

    #[test]
    fn convenience_aliases_canonicalise_with_fixed_recipe_facts() {
        let registry = load_action_registry().expect("registry");
        let expected = [
            ("MONEY_IN.DAILY_TAKINGS", "MONEY_IN.SAVE", "category", "Sales/Trading"),
            ("MONEY_OUT.MIXED_USE", "MONEY_OUT.SAVE", "business_use", "mixed"),
            ("MONEY_OUT.PRIVATE_FROM_BUSINESS_FUNDS", "MONEY_OUT.SAVE", "business_use", "private"),
            ("CONTACT.CREATE_CUSTOMER", "CONTACTS.SAVE", "kind", "customer"),
            ("CONTACT.CREATE_SUPPLIER", "CONTACTS.SAVE", "kind", "supplier"),
        ];
        for (alias, target, slot, value) in expected {
            assert_eq!(registry.canonical_action_id(alias), Some(target));
            assert_eq!(registry.alias(alias).expect("alias").preset.get(slot).map(String::as_str), Some(value));
            assert!(registry.action(alias).is_none());
        }
    }

    #[test]
    fn no_candidate_is_unknown() {
        let controller = controller(&[]);
        let outcome = controller.resolve(&ResolutionRequest { candidate_action_ids: Vec::new(), facts: Vec::new(), invocation_source: InvocationSource::VoiceTranscript });
        assert_eq!(outcome.state, ResolutionState::Unknown);
        assert_eq!(outcome.attention.expect("attention").reason_code, AttentionReason::UnknownRequest);
    }

    #[test]
    fn near_neighbour_open_books_is_ambiguous() {
        let controller = controller(&["BOOKS.OPEN"]);
        let outcome = controller.resolve(&ResolutionRequest {
            candidate_action_ids: vec!["BOOKS.OPEN".to_string(), "NAVIGATION.BOOKS_HOME".to_string()],
            facts: Vec::new(),
            invocation_source: InvocationSource::VoiceTranscript,
        });
        assert_eq!(outcome.state, ResolutionState::Ambiguous);
        assert_eq!(outcome.action_id, None);
    }

    #[test]
    fn locked_action_is_known_but_not_executable() {
        let controller = controller(&[]);
        let outcome = controller.resolve(&ResolutionRequest { candidate_action_ids: vec!["OWNER.DRAWING.RECORD".to_string()], facts: Vec::new(), invocation_source: InvocationSource::CommandText });
        assert_eq!(outcome.state, ResolutionState::Known);
        assert_eq!(outcome.execution, ExecutionAvailability::Locked);
        assert_eq!(outcome.attention.expect("attention").reason_code, AttentionReason::LockedAction);
    }

    #[test]
    fn missing_owner_slots_are_requested_but_context_is_not_relabelled_owner_input() {
        let controller = controller(&["CONTACTS.SAVE"]);
        let outcome = controller.resolve(&ResolutionRequest {
            candidate_action_ids: vec!["CONTACT.CREATE_CUSTOMER".to_string()],
            facts: vec![fact("contact_id", "contact-1")],
            invocation_source: InvocationSource::Manual,
        });
        assert_eq!(outcome.state, ResolutionState::Unknown);
        assert_eq!(outcome.execution, ExecutionAvailability::Executable);
        assert_eq!(outcome.missing_owner_slots, vec!["display_name".to_string()]);
        assert!(outcome.missing_context_slots.is_empty());
    }

    #[test]
    fn missing_context_is_distinct_from_owner_question() {
        let controller = controller(&["BOOKS.OPEN"]);
        let outcome = controller.resolve(&ResolutionRequest { candidate_action_ids: vec!["BOOKS.OPEN".to_string()], facts: Vec::new(), invocation_source: InvocationSource::CommandText });
        assert_eq!(outcome.state, ResolutionState::Unknown);
        assert!(outcome.missing_owner_slots.is_empty());
        assert_eq!(outcome.missing_context_slots, vec!["books_reference".to_string(), "actor".to_string()]);
        assert_eq!(outcome.attention.expect("attention").reason_code, AttentionReason::MissingContext);
    }

    #[test]
    fn conflicting_owner_facts_fail_closed() {
        let controller = controller(&["CONTACTS.SAVE"]);
        let outcome = controller.resolve(&ResolutionRequest {
            candidate_action_ids: vec!["CONTACTS.SAVE".to_string()],
            facts: vec![fact("contact_id", "contact-1"), fact("kind", "customer"), fact("kind", "supplier"), fact("display_name", "Example")],
            invocation_source: InvocationSource::VoiceTranscript,
        });
        assert_eq!(outcome.state, ResolutionState::Conflicting);
        assert_eq!(outcome.conflicts.len(), 1);
        assert_eq!(outcome.conflicts[0].slot_id, "kind");
    }

    #[test]
    fn alias_recipe_conflict_with_owner_fact_fails_closed() {
        let controller = controller(&["CONTACTS.SAVE"]);
        let outcome = controller.resolve(&ResolutionRequest {
            candidate_action_ids: vec!["CONTACT.CREATE_CUSTOMER".to_string()],
            facts: vec![fact("contact_id", "contact-1"), fact("kind", "supplier"), fact("display_name", "Example")],
            invocation_source: InvocationSource::CommandText,
        });
        assert_eq!(outcome.state, ResolutionState::Conflicting);
        assert_eq!(outcome.conflicts[0].slot_id, "kind");
    }

    #[test]
    fn alias_recipe_supplies_fixed_fact_without_new_authority() {
        let controller = controller(&["CONTACTS.SAVE"]);
        let outcome = controller.resolve(&ResolutionRequest {
            candidate_action_ids: vec!["CONTACT.CREATE_CUSTOMER".to_string()],
            facts: vec![fact("contact_id", "contact-1"), fact("display_name", "Example")],
            invocation_source: InvocationSource::CommandText,
        });
        assert_eq!(outcome.state, ResolutionState::Known);
        assert_eq!(outcome.execution, ExecutionAvailability::Executable);
        assert_eq!(outcome.action_id.as_deref(), Some("CONTACTS.SAVE"));
        assert!(outcome.attention.is_none());
        assert!(outcome.resolved_facts.iter().any(|fact| fact.slot_id == "kind" && fact.value == "customer" && fact.source == FactSource::AliasRecipe));
    }

    #[test]
    fn confirmation_cannot_be_lowered_by_surface() {
        let controller = controller(&["MONEY_OUT.SAVE"]);
        let facts = vec![
            fact("record_id", "expense-1"), fact("description", "Fuel"), fact("date", "2026-09-15"),
            fact("positive_gbp_pence", "4800"), fact("category", "MOTOR"), fact("business_use", "business"),
            fact("settlement", "BUSINESS_BANK"),
        ];
        assert!(controller.create_confirmation("MONEY_OUT.SAVE", "state-1", &facts, ConfirmationClass::None).is_err());
        controller.create_confirmation("MONEY_OUT.SAVE", "state-1", &facts, ConfirmationClass::Explicit).expect("explicit confirmation");
    }

    #[test]
    fn confirmation_fails_when_state_or_facts_change() {
        let controller = controller(&["CONTACTS.SAVE"]);
        let facts = vec![fact("contact_id", "contact-1"), fact("kind", "customer"), fact("display_name", "Example")];
        let receipt = controller.create_confirmation("CONTACTS.SAVE", "revision-1", &facts, ConfirmationClass::Explicit).expect("receipt");
        controller.validate_confirmation(&receipt, "revision-1", &facts).expect("current");
        assert!(controller.validate_confirmation(&receipt, "revision-2", &facts).is_err());
        let changed = vec![fact("contact_id", "contact-1"), fact("kind", "supplier"), fact("display_name", "Example")];
        assert!(controller.validate_confirmation(&receipt, "revision-1", &changed).is_err());
    }

    #[test]
    fn replay_is_consumed_only_after_current_confirmation_passes() {
        let controller = controller(&["CONTACTS.SAVE"]);
        let resolution = controller.resolve(&ResolutionRequest {
            candidate_action_ids: vec!["CONTACT.CREATE_CUSTOMER".to_string()],
            facts: vec![fact("contact_id", "contact-1"), fact("display_name", "Example")],
            invocation_source: InvocationSource::VoiceTranscript,
        });
        let call = controller.prepare_call(&resolution, "operation-1", InvocationSource::VoiceTranscript, "revision-1").expect("call");
        let receipt = controller.create_confirmation(
            "CONTACT.CREATE_CUSTOMER", "revision-1",
            &[fact("contact_id", "contact-1"), fact("display_name", "Example")],
            ConfirmationClass::Explicit,
        ).expect("receipt");
        let mut guard = ReplayGuard::default();
        assert!(controller.accept_call(&call, "revision-1", None, &mut guard).is_err());
        controller.accept_call(&call, "revision-1", Some(&receipt), &mut guard).expect("confirmed first acceptance");
        let replay = controller.accept_call(&call, "revision-1", Some(&receipt), &mut guard).expect_err("duplicate blocked");
        assert!(replay.message.contains("REPLAY_DETECTED"));
    }

    #[test]
    fn stale_state_fails_before_replay_id_is_consumed() {
        let controller = controller(&["REPORT.SUMMARY"]);
        let resolution = controller.resolve(&ResolutionRequest {
            candidate_action_ids: vec!["REPORT.SUMMARY".to_string()],
            facts: vec![context_fact("books_reference", "current-books")],
            invocation_source: InvocationSource::Manual,
        });
        let call = controller.prepare_call(&resolution, "report-op", InvocationSource::Manual, "revision-1").expect("call");
        let mut guard = ReplayGuard::default();
        assert!(controller.accept_call(&call, "revision-2", None, &mut guard).is_err());
        controller.accept_call(&call, "revision-1", None, &mut guard).expect("fresh retry accepted");
    }

    #[test]
    fn attention_ids_are_deterministic() {
        let controller = controller(&[]);
        let request = ResolutionRequest {
            candidate_action_ids: vec!["BOOKS.OPEN".to_string(), "NAVIGATION.BOOKS_HOME".to_string()],
            facts: Vec::new(),
            invocation_source: InvocationSource::VoiceTranscript,
        };
        let first = controller.resolve(&request).attention.expect("first attention").attention_id;
        let second = controller.resolve(&request).attention.expect("second attention").attention_id;
        assert_eq!(first, second);
    }
}
