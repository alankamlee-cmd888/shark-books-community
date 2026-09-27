use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use shark_foundation::action_system::{
    finite_command_candidates, load_action_registry, ActionController, ActionFact, AttentionItem,
    AttentionReason, ExecutionAvailability, FactSource, ResolutionOutcome, ResolutionState,
    SlotSource,
};

// R4 reconciles the five proven R1/R2 read/view bridges to READY in the same canonical registry.
// Command text still gains no authority beyond the existing bounded manual/native routes.
pub(crate) const COMMAND_TEXT_EXPOSED_ACTION_IDS: &[&str] = &[
    "BOOKS.CREATE",
    "BOOKS.OPEN",
    "BOOKS.VERIFY",
    "HOME.STATUS",
    "MONEY_IN.PREVIEW",
    "MONEY_IN.SAVE",
    "MONEY_OUT.PREVIEW",
    "MONEY_OUT.SAVE",
    "MONEY.RECORDS.LIST",
    "MONEY.RECORD.DETAIL",
    "CORRECTION.PREVIEW",
    "CORRECTION.CONFIRM",
    "CORRECTION.HISTORY",
    "BANK.IMPORT_PREVIEW_CSV",
    "BANK.IMPORT_PREVIEW_OFX_QFX",
    "BANK.IMPORT_CONFIRM_CSV",
    "BANK.IMPORT_CONFIRM_OFX_QFX",
    "BANK.ACTIVITY_LIST",
    "BANK.ACTIVITY_DETAIL",
    "BANK.MATCH_REVIEW",
    "BANK.MATCH_CONFIRM",
    "BANK.RECONCILE_PREVIEW",
    "BANK.RECONCILE_FINALISE",
    "DOCUMENT.SELECT_REGISTER",
    "DOCUMENT.VERIFY",
    "DOCUMENT.LIST",
    "DOCUMENT.OPEN_VIEW",
    "DOCUMENT.ATTACH",
    "OCR.EXTRACT_RECEIPT",
    "RECEIPT.SUGGEST_BANK",
    "RECEIPT.CONFIRM_BANK",
    "RECEIPT.REJECT_BANK",
    "CONTACTS.LIST",
    "CONTACTS.SAVE",
    "REPORT.SUMMARY",
    "SETTINGS.BOOKS_INFO",
    "SETTINGS.STORAGE_ROOT.SELECT",
];

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct OwnerCommandFactInput {
    slot_id: String,
    value: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct OwnerCommandTextRequest {
    text: String,
    books_reference: Option<String>,
    actor: Option<String>,
    candidate_action_id: Option<String>,
    #[serde(default)]
    facts: Vec<OwnerCommandFactInput>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OwnerCommandChoice {
    action_id: String,
    manual_label: String,
    family: String,
    confirmation_class: String,
    backend_state: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OwnerCommandPrompt {
    slot_id: String,
    label: String,
    choices: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OwnerCommandAttention {
    attention_id: String,
    reason_code: String,
    title: String,
    summary: String,
    action_ids: Vec<String>,
    allowed_next_action_ids: Vec<String>,
    missing_owner_slots: Vec<String>,
    missing_context_slots: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OwnerCommandResolution {
    state: String,
    action_id: Option<String>,
    execution: String,
    resolved_facts: Vec<OwnerCommandResolvedFact>,
    missing_owner_slots: Vec<String>,
    missing_context_slots: Vec<String>,
    attention: Option<OwnerCommandAttention>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OwnerCommandResolvedFact {
    slot_id: String,
    value: String,
    source: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OwnerCommandTextResponse {
    candidate_action_ids: Vec<String>,
    choices: Vec<OwnerCommandChoice>,
    resolution: OwnerCommandResolution,
    prompt: Option<OwnerCommandPrompt>,
}

fn bounded_text(label: &str, value: &str, max_chars: usize) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(format!("{label} must not be empty"));
    }
    if trimmed.chars().count() > max_chars {
        return Err(format!("{label} is too long"));
    }
    Ok(trimmed.to_string())
}

fn resolution_state(value: ResolutionState) -> &'static str {
    match value {
        ResolutionState::Known => "KNOWN",
        ResolutionState::Unknown => "UNKNOWN",
        ResolutionState::Ambiguous => "AMBIGUOUS",
        ResolutionState::Conflicting => "CONFLICTING",
    }
}

fn execution_state(value: ExecutionAvailability) -> &'static str {
    match value {
        ExecutionAvailability::Executable => "EXECUTABLE",
        ExecutionAvailability::Locked => "LOCKED",
        ExecutionAvailability::InternalOnly => "INTERNAL_ONLY",
        ExecutionAvailability::NotAuthorised => "NOT_AUTHORISED",
    }
}

fn attention_reason(value: AttentionReason) -> &'static str {
    match value {
        AttentionReason::UnknownRequest => "UNKNOWN_REQUEST",
        AttentionReason::MissingInformation => "MISSING_INFORMATION",
        AttentionReason::MissingContext => "MISSING_CONTEXT",
        AttentionReason::AmbiguousAction => "AMBIGUOUS_ACTION",
        AttentionReason::ConflictingInformation => "CONFLICTING_INFORMATION",
        AttentionReason::LockedAction => "LOCKED_ACTION",
        AttentionReason::InternalOnly => "INTERNAL_ONLY",
        AttentionReason::NotAuthorised => "NOT_AUTHORISED",
        AttentionReason::StaleConfirmation => "STALE_CONFIRMATION",
        AttentionReason::ReplayDetected => "REPLAY_DETECTED",
    }
}

fn fact_source(value: FactSource) -> &'static str {
    match value {
        FactSource::Owner => "OWNER",
        FactSource::SystemContext => "SYSTEM_CONTEXT",
        FactSource::Ocr => "OCR",
        FactSource::Interpreter => "INTERPRETER",
        FactSource::AliasRecipe => "ALIAS_RECIPE",
    }
}

fn attention_view(attention: AttentionItem) -> OwnerCommandAttention {
    OwnerCommandAttention {
        attention_id: attention.attention_id,
        reason_code: attention_reason(attention.reason_code).to_string(),
        title: attention.title,
        summary: attention.summary,
        action_ids: attention.action_ids,
        allowed_next_action_ids: attention.allowed_next_action_ids,
        missing_owner_slots: attention.missing_owner_slots,
        missing_context_slots: attention.missing_context_slots,
    }
}

fn resolution_view(outcome: ResolutionOutcome) -> OwnerCommandResolution {
    OwnerCommandResolution {
        state: resolution_state(outcome.state).to_string(),
        action_id: outcome.action_id,
        execution: execution_state(outcome.execution).to_string(),
        resolved_facts: outcome
            .resolved_facts
            .into_iter()
            .map(|fact| OwnerCommandResolvedFact {
                slot_id: fact.slot_id,
                value: fact.value,
                source: fact_source(fact.source).to_string(),
            })
            .collect(),
        missing_owner_slots: outcome.missing_owner_slots,
        missing_context_slots: outcome.missing_context_slots,
        attention: outcome.attention.map(attention_view),
    }
}

fn build_context_facts(
    books_reference: Option<&str>,
    actor: Option<&str>,
) -> Result<Vec<ActionFact>, String> {
    let mut facts = Vec::new();
    if let Some(value) = books_reference {
        let value = bounded_text("books reference", value, 120)?;
        for slot_id in ["books_reference", "books", "books_name_id"] {
            facts.push(
                ActionFact::new(slot_id, value.clone(), FactSource::SystemContext)
                    .map_err(|error| error.to_string())?,
            );
        }
    }
    if let Some(value) = actor {
        let value = bounded_text("actor", value, 100)?;
        facts.push(
            ActionFact::new("actor", value, FactSource::SystemContext)
                .map_err(|error| error.to_string())?,
        );
    }
    Ok(facts)
}

fn canonical_candidate_set(
    registry: &shark_foundation::action_system::ActionRegistryDocument,
    candidates: &[String],
) -> BTreeSet<String> {
    candidates
        .iter()
        .filter_map(|candidate| registry.canonical_action_id(candidate).map(ToOwned::to_owned))
        .collect()
}

fn resolve_impl(request: OwnerCommandTextRequest) -> Result<OwnerCommandTextResponse, String> {
    let text = bounded_text("command text", &request.text, 500)?;
    if request.facts.len() > 20 {
        return Err("too many command clarification facts".to_string());
    }

    let registry = load_action_registry().map_err(|error| error.to_string())?;
    let finite_candidates = finite_command_candidates(&registry, &text);
    let finite_canonical = canonical_candidate_set(&registry, &finite_candidates);

    let candidate_action_ids = if let Some(selected) = request.candidate_action_id.as_deref() {
        let selected = bounded_text("candidate Action ID", selected, 100)?;
        let canonical = registry
            .canonical_action_id(&selected)
            .ok_or_else(|| "selected Action ID is not in the Shark Action Registry".to_string())?;
        if !finite_canonical.contains(canonical) {
            return Err(
                "selected Action ID is not a candidate for the current command text".to_string(),
            );
        }
        let matching_raw: Vec<String> = finite_candidates
            .iter()
            .filter(|candidate| registry.canonical_action_id(candidate) == Some(canonical))
            .cloned()
            .collect();
        if matching_raw.len() == 1 {
            matching_raw
        } else {
            vec![selected]
        }
    } else {
        finite_candidates.clone()
    };

    let canonical = canonical_candidate_set(&registry, &candidate_action_ids);
    if !request.facts.is_empty() && canonical.len() != 1 {
        return Err(
            "clarification facts require exactly one selected canonical Action ID".to_string(),
        );
    }

    let selected_action = canonical
        .iter()
        .next()
        .and_then(|action_id| registry.action(action_id));

    let mut facts = build_context_facts(
        request.books_reference.as_deref(),
        request.actor.as_deref(),
    )?;

    if let Some(action) = selected_action {
        let owner_slots: BTreeSet<&str> = action
            .required_slots
            .iter()
            .filter(|slot| slot.source == SlotSource::Owner)
            .map(|slot| slot.slot_id.as_str())
            .collect();
        for input in request.facts {
            let slot_id = bounded_text("slot id", &input.slot_id, 100)?;
            let value = bounded_text("slot value", &input.value, 300)?;
            if !owner_slots.contains(slot_id.as_str()) {
                return Err(format!(
                    "slot {slot_id} is not an owner-supplied slot for the selected Action ID"
                ));
            }
            facts.push(
                ActionFact::new(slot_id, value, FactSource::Owner)
                    .map_err(|error| error.to_string())?,
            );
        }
    } else if !request.facts.is_empty() {
        return Err("clarification facts cannot be supplied before an Action ID is selected".into());
    }

    let controller = ActionController::new(
        registry,
        COMMAND_TEXT_EXPOSED_ACTION_IDS.iter().copied(),
    )
    .map_err(|error| error.to_string())?;
    let outcome = controller.resolve(
        &shark_foundation::action_system::ResolutionRequest {
            candidate_action_ids: candidate_action_ids.clone(),
            facts,
            invocation_source: shark_foundation::action_system::InvocationSource::CommandText,
        },
    );

    let mut choices = Vec::new();
    let mut seen = BTreeSet::new();
    for candidate in &finite_candidates {
        let Some(canonical_id) = controller.registry().canonical_action_id(candidate) else {
            continue;
        };
        if !seen.insert(canonical_id.to_string()) {
            continue;
        }
        let action = controller
            .registry()
            .action(canonical_id)
            .expect("canonical Action ID is present");
        choices.push(OwnerCommandChoice {
            action_id: action.action_id.clone(),
            manual_label: action.manual_label.clone(),
            family: action.family.clone(),
            confirmation_class: format!("{:?}", action.confirmation_class).to_ascii_uppercase(),
            backend_state: action.backend_state.clone(),
        });
    }

    let prompt = outcome.action_id.as_deref().and_then(|action_id| {
        let action = controller.registry().action(action_id)?;
        let slot_id = outcome.missing_owner_slots.first()?;
        let slot = action.required_slots.iter().find(|slot| &slot.slot_id == slot_id)?;
        let mut choices = slot.choices.clone();
        for hint in &action.choice_hints {
            if !choices.contains(hint) {
                choices.push(hint.clone());
            }
        }
        Some(OwnerCommandPrompt {
            slot_id: slot.slot_id.clone(),
            label: slot.label.clone(),
            choices,
        })
    });

    Ok(OwnerCommandTextResponse {
        candidate_action_ids,
        choices,
        resolution: resolution_view(outcome),
        prompt,
    })
}

#[tauri::command]
pub(crate) fn owner_command_text_resolve(
    request: OwnerCommandTextRequest,
) -> Result<OwnerCommandTextResponse, String> {
    resolve_impl(request)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposure_set_is_exact_and_unique() {
        assert_eq!(COMMAND_TEXT_EXPOSED_ACTION_IDS.len(), 37);
        let unique: BTreeSet<&str> = COMMAND_TEXT_EXPOSED_ACTION_IDS.iter().copied().collect();
        assert_eq!(unique.len(), 37);
    }

    #[test]
    fn locked_future_action_is_known_but_not_executable() {
        let registry = load_action_registry().expect("registry");
        let action = registry
            .actions
            .iter()
            .find(|action| {
                action.voice_eligible
                    && action.backend_state != "READY"
                    && !action.utterances.is_empty()
            })
            .expect("locked voice action");
        let text = action.utterances[0].clone();
        let response = resolve_impl(OwnerCommandTextRequest {
            text,
            books_reference: Some("current-books".to_string()),
            actor: Some("owner".to_string()),
            candidate_action_id: None,
            facts: Vec::new(),
        })
        .expect("resolution");
        assert_ne!(response.resolution.execution, "EXECUTABLE");
    }

    #[test]
    fn report_summary_is_command_text_executable_with_context() {
        let response = resolve_impl(OwnerCommandTextRequest {
            text: "Business summary".to_string(),
            books_reference: Some("current-books".to_string()),
            actor: Some("owner".to_string()),
            candidate_action_id: None,
            facts: Vec::new(),
        })
        .expect("resolution");
        assert_eq!(response.resolution.action_id.as_deref(), Some("REPORT.SUMMARY"));
        assert_eq!(response.resolution.state, "KNOWN");
        assert_eq!(response.resolution.execution, "EXECUTABLE");
    }

    #[test]
    fn reconciled_document_open_is_command_text_executable_with_owner_fact() {
        let response = resolve_impl(OwnerCommandTextRequest {
            text: "Open receipt/document".to_string(),
            books_reference: Some("current-books".to_string()),
            actor: Some("owner".to_string()),
            candidate_action_id: None,
            facts: vec![OwnerCommandFactInput {
                slot_id: "document_id".to_string(),
                value: "doc-1".to_string(),
            }],
        })
        .expect("resolution");
        assert_eq!(response.resolution.action_id.as_deref(), Some("DOCUMENT.OPEN_VIEW"));
        assert_eq!(response.resolution.state, "KNOWN");
        assert_eq!(response.resolution.execution, "EXECUTABLE");
    }

    #[test]
    fn forged_ambiguity_selection_is_rejected() {
        let error = resolve_impl(OwnerCommandTextRequest {
            text: "Open my books".to_string(),
            books_reference: Some("current-books".to_string()),
            actor: Some("owner".to_string()),
            candidate_action_id: Some("REPORT.SUMMARY".to_string()),
            facts: Vec::new(),
        })
        .expect_err("forged selection must fail");
        assert!(error.contains("not a candidate"));
    }

    #[test]
    fn arbitrary_owner_fact_slot_is_rejected() {
        let error = resolve_impl(OwnerCommandTextRequest {
            text: "Save contact".to_string(),
            books_reference: Some("current-books".to_string()),
            actor: Some("owner".to_string()),
            candidate_action_id: Some("CONTACTS.SAVE".to_string()),
            facts: vec![OwnerCommandFactInput {
                slot_id: "unexpected_slot".to_string(),
                value: "unexpected".to_string(),
            }],
        })
        .expect_err("unknown slot must fail");
        assert!(error.contains("not an owner-supplied slot"));
    }
}
