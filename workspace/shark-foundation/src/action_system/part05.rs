/// Deterministic finite command-text matching over the frozen Action Registry.
///
/// This layer performs routing only. It does not execute an ActionCall and it
/// deliberately has no fuzzy/model/network path.
pub fn normalize_command_text(input: &str) -> String {
    input
        .trim()
        .trim_matches(|character: char| matches!(character, '.' | '!' | '?'))
        .split_whitespace()
        .map(|part| part.to_lowercase())
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn finite_command_candidates(
    registry: &ActionRegistryDocument,
    input: &str,
) -> Vec<String> {
    let needle = normalize_command_text(input);
    if needle.is_empty() {
        return Vec::new();
    }

    let mut matches = BTreeSet::new();
    for action in &registry.actions {
        if !action.voice_eligible {
            continue;
        }

        if normalize_command_text(&action.manual_label) == needle
            || normalize_command_text(&action.owner_intent) == needle
            || action
                .utterances
                .iter()
                .any(|utterance| normalize_command_text(utterance) == needle)
        {
            matches.insert(action.action_id.clone());
        }
    }

    for alias in &registry.aliases {
        if normalize_command_text(&alias.label) == needle {
            matches.insert(alias.alias_action_id.clone());
        }
    }

    matches.into_iter().collect()
}

pub fn resolve_command_text(
    controller: &ActionController,
    input: &str,
    facts: Vec<ActionFact>,
) -> ResolutionOutcome {
    let candidates = finite_command_candidates(controller.registry(), input);
    controller.resolve(&ResolutionRequest {
        candidate_action_ids: candidates,
        facts,
        invocation_source: InvocationSource::CommandText,
    })
}

#[cfg(test)]
mod command_text_tests {
    use super::*;

    fn context(slot_id: &str, value: &str) -> ActionFact {
        ActionFact::new(slot_id, value, FactSource::SystemContext).expect("context fact")
    }

    #[test]
    fn every_frozen_utterance_retains_its_canonical_candidate() {
        let registry = load_action_registry().expect("registry");
        let mut checked = 0usize;
        for action in registry.actions.iter().filter(|action| action.voice_eligible) {
            for utterance in &action.utterances {
                let candidates = finite_command_candidates(&registry, utterance);
                assert!(
                    candidates.iter().any(|candidate| {
                        registry.canonical_action_id(candidate)
                            == Some(action.action_id.as_str())
                    }),
                    "fixture no longer routes to {}: {:?}",
                    action.action_id,
                    utterance
                );
                checked += 1;
            }
        }
        assert_eq!(checked, 700);
    }

    #[test]
    fn open_my_books_remains_ambiguous() {
        let registry = load_action_registry().expect("registry");
        let candidates = finite_command_candidates(&registry, "Open my books");
        let canonical: BTreeSet<String> = candidates
            .iter()
            .filter_map(|candidate| registry.canonical_action_id(candidate).map(ToOwned::to_owned))
            .collect();
        assert!(canonical.contains("BOOKS.OPEN"));
        assert!(canonical.contains("NAVIGATION.BOOKS_HOME"));

        let controller = ActionController::new(registry, ["BOOKS.OPEN"]).expect("controller");
        let outcome = controller.resolve(&ResolutionRequest {
            candidate_action_ids: candidates,
            facts: Vec::new(),
            invocation_source: InvocationSource::CommandText,
        });
        assert_eq!(outcome.state, ResolutionState::Ambiguous);
        assert_eq!(
            outcome.attention.expect("attention").reason_code,
            AttentionReason::AmbiguousAction
        );
    }

    #[test]
    fn current_ready_action_routes_through_existing_controller() {
        let controller = ActionController::new(
            load_action_registry().expect("registry"),
            ["REPORT.SUMMARY"],
        )
        .expect("controller");
        let outcome = resolve_command_text(
            &controller,
            "Business summary",
            vec![context("books_reference", "current-books")],
        );
        assert_eq!(outcome.state, ResolutionState::Known);
        assert_eq!(outcome.execution, ExecutionAvailability::Executable);
        assert_eq!(outcome.action_id.as_deref(), Some("REPORT.SUMMARY"));
    }

    #[test]
    fn exposed_but_stale_locked_read_view_action_stays_fail_closed() {
        let controller = ActionController::new(
            load_action_registry().expect("registry"),
            ["DOCUMENT.OPEN_VIEW"],
        )
        .expect("controller");
        let outcome = resolve_command_text(
            &controller,
            "Open receipt/document",
            Vec::new(),
        );
        assert_eq!(outcome.state, ResolutionState::Known);
        assert_eq!(outcome.execution, ExecutionAvailability::Locked);
        assert_eq!(
            outcome.attention.expect("attention").reason_code,
            AttentionReason::LockedAction
        );
    }

    #[test]
    fn convenience_alias_label_preserves_recipe_fact() {
        let registry = load_action_registry().expect("registry");
        let alias = registry
            .aliases
            .iter()
            .find(|alias| alias.alias_action_id == "CONTACT.CREATE_CUSTOMER")
            .expect("customer alias");
        let input = alias.label.clone();
        let controller =
            ActionController::new(registry, ["CONTACTS.SAVE"]).expect("controller");
        let outcome = resolve_command_text(&controller, &input, Vec::new());
        assert_eq!(outcome.action_id.as_deref(), Some("CONTACTS.SAVE"));
        assert!(outcome.resolved_facts.iter().any(|fact| {
            fact.slot_id == "kind"
                && fact.value == "customer"
                && fact.source == FactSource::AliasRecipe
        }));
    }

    #[test]
    fn unknown_text_stays_unknown_without_fuzzy_fallback() {
        let controller =
            ActionController::new(load_action_registry().expect("registry"), ["REPORT.SUMMARY"])
                .expect("controller");
        let outcome = resolve_command_text(
            &controller,
            "please invent a brand new accounting capability for me",
            Vec::new(),
        );
        assert_eq!(outcome.state, ResolutionState::Unknown);
        assert!(outcome.action_id.is_none());
    }
}
