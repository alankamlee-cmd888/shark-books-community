#[derive(Debug, Clone, Default)]
pub struct ReplayGuard {
    seen_operation_ids: BTreeSet<String>,
}

impl ReplayGuard {
    pub fn register(&mut self, operation_id: impl Into<String>) -> FoundationResult<()> {
        let operation_id = operation_id.into();
        if operation_id.trim().is_empty() {
            return Err(validation("operation id must not be empty"));
        }
        if !self.seen_operation_ids.insert(operation_id) {
            return Err(FoundationError::new(
                FoundationErrorCode::Validation,
                "REPLAY_DETECTED: operation id has already been accepted",
            ));
        }
        Ok(())
    }
}

fn detect_conflicts(facts: &[ActionFact]) -> Vec<ConflictDetail> {
    let mut values_by_slot: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for fact in facts {
        values_by_slot
            .entry(fact.slot_id.trim())
            .or_default()
            .insert(fact.value.trim());
    }
    values_by_slot
        .into_iter()
        .filter_map(|(slot_id, values)| {
            if values.len() <= 1 {
                return None;
            }
            Some(ConflictDetail {
                slot_id: slot_id.to_string(),
                values: values.into_iter().map(ToOwned::to_owned).collect(),
            })
        })
        .collect()
}

fn facts_fingerprint(facts: &[ActionFact]) -> String {
    let mut rows: Vec<String> = facts
        .iter()
        .map(|fact| {
            format!(
                "{:?}\u{1f}{}\u{1f}{}",
                fact.source,
                fact.slot_id.trim(),
                fact.value.trim()
            )
        })
        .collect();
    rows.sort();
    let mut digest = Sha256::new();
    for row in rows {
        digest.update(row.as_bytes());
        digest.update([0u8]);
    }
    format!("{:x}", digest.finalize())
}

fn make_attention(
    reason: AttentionReason,
    state: ResolutionState,
    summary: &str,
    action_ids: Vec<String>,
    allowed_next_action_ids: Vec<String>,
    missing_owner_slots: Vec<String>,
    missing_context_slots: Vec<String>,
    conflicts: Vec<ConflictDetail>,
) -> AttentionItem {
    let mut material = format!("{reason:?}|{state:?}|{summary}");
    for action_id in &action_ids {
        material.push('|');
        material.push_str(action_id);
    }
    for slot in &missing_owner_slots {
        material.push('|');
        material.push_str("owner:");
        material.push_str(slot);
    }
    for slot in &missing_context_slots {
        material.push('|');
        material.push_str("context:");
        material.push_str(slot);
    }
    for conflict in &conflicts {
        material.push('|');
        material.push_str(&conflict.slot_id);
        for value in &conflict.values {
            material.push('|');
            material.push_str(value);
        }
    }
    let hash = Sha256::digest(material.as_bytes());
    let attention_id = format!("attention-{:x}", hash);
    AttentionItem {
        attention_id: attention_id[..26].to_string(),
        reason_code: reason,
        title: attention_title(reason).to_string(),
        summary: summary.to_string(),
        action_ids,
        allowed_next_action_ids,
        missing_owner_slots,
        missing_context_slots,
        conflicts,
        resolution_state: state,
    }
}

fn attention_title(reason: AttentionReason) -> &'static str {
    match reason {
        AttentionReason::UnknownRequest => "Action not identified",
        AttentionReason::MissingInformation => "More information needed",
        AttentionReason::MissingContext => "App context needed",
        AttentionReason::AmbiguousAction => "Choose an action",
        AttentionReason::ConflictingInformation => "Resolve conflicting information",
        AttentionReason::LockedAction => "Action not available yet",
        AttentionReason::InternalOnly => "Internal operation",
        AttentionReason::NotAuthorised => "Action not authorised",
        AttentionReason::StaleConfirmation => "Confirmation expired",
        AttentionReason::ReplayDetected => "Duplicate action blocked",
    }
}

fn validation(message: impl Into<String>) -> FoundationError {
    FoundationError::new(FoundationErrorCode::Validation, message)
}

fn stale_confirmation(message: impl Into<String>) -> FoundationError {
    FoundationError::new(
        FoundationErrorCode::Validation,
        format!("STALE_CONFIRMATION: {}", message.into()),
    )
}
