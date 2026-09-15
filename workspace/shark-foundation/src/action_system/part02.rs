#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum InvocationSource {
    Manual,
    CommandText,
    VoiceTranscript,
    CameraDocument,
    PlatformIntent,
    Integration,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FactSource {
    Owner,
    SystemContext,
    Ocr,
    Interpreter,
    AliasRecipe,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActionFact {
    pub slot_id: String,
    pub value: String,
    pub source: FactSource,
}

impl ActionFact {
    pub fn new(
        slot_id: impl Into<String>,
        value: impl Into<String>,
        source: FactSource,
    ) -> FoundationResult<Self> {
        let slot_id = slot_id.into();
        let value = value.into();
        if slot_id.trim().is_empty() || value.trim().is_empty() {
            return Err(validation("Action fact slot/value must not be empty"));
        }
        Ok(Self {
            slot_id,
            value,
            source,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResolutionRequest {
    pub candidate_action_ids: Vec<String>,
    #[serde(default)]
    pub facts: Vec<ActionFact>,
    pub invocation_source: InvocationSource,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ResolutionState {
    Known,
    Unknown,
    Ambiguous,
    Conflicting,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExecutionAvailability {
    Executable,
    Locked,
    InternalOnly,
    NotAuthorised,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConflictDetail {
    pub slot_id: String,
    pub values: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AttentionReason {
    UnknownRequest,
    MissingInformation,
    MissingContext,
    AmbiguousAction,
    ConflictingInformation,
    LockedAction,
    InternalOnly,
    NotAuthorised,
    StaleConfirmation,
    ReplayDetected,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AttentionItem {
    pub attention_id: String,
    pub reason_code: AttentionReason,
    pub title: String,
    pub summary: String,
    pub action_ids: Vec<String>,
    pub allowed_next_action_ids: Vec<String>,
    pub missing_owner_slots: Vec<String>,
    pub missing_context_slots: Vec<String>,
    pub conflicts: Vec<ConflictDetail>,
    pub resolution_state: ResolutionState,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResolutionOutcome {
    pub state: ResolutionState,
    pub action_id: Option<String>,
    pub execution: ExecutionAvailability,
    pub resolved_facts: Vec<ActionFact>,
    pub missing_owner_slots: Vec<String>,
    pub missing_context_slots: Vec<String>,
    pub conflicts: Vec<ConflictDetail>,
    pub attention: Option<AttentionItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActionCall {
    pub action_id: String,
    pub action_version: u32,
    pub operation_id: String,
    pub invocation_source: InvocationSource,
    pub state_revision: String,
    pub facts: Vec<ActionFact>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConfirmationReceipt {
    pub action_id: String,
    pub action_version: u32,
    pub state_revision: String,
    pub facts_fingerprint: String,
    pub confirmation_class: ConfirmationClass,
}

#[derive(Debug, Clone)]
pub struct ActionController {
    registry: ActionRegistryDocument,
    exposed_action_ids: BTreeSet<String>,
}

impl ActionController {
    pub fn new<I, S>(registry: ActionRegistryDocument, exposed_action_ids: I) -> FoundationResult<Self>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        registry.validate()?;
        let mut exposed = BTreeSet::new();
        for action_id in exposed_action_ids {
            let action_id = action_id.into();
            let canonical = registry
                .canonical_action_id(&action_id)
                .ok_or_else(|| validation(format!("unknown exposed Action ID: {action_id}")))?;
            exposed.insert(canonical.to_string());
        }
        Ok(Self {
            registry,
            exposed_action_ids: exposed,
        })
    }

    pub fn registry(&self) -> &ActionRegistryDocument {
        &self.registry
    }

    pub fn resolve(&self, request: &ResolutionRequest) -> ResolutionOutcome {
        let (canonical_ids, alias_facts) =
            self.canonical_candidates_and_alias_facts(&request.candidate_action_ids);
        if canonical_ids.is_empty() {
            return self.outcome_with_attention(
                ResolutionState::Unknown,
                None,
                ExecutionAvailability::Locked,
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                AttentionReason::UnknownRequest,
                "I could not identify a SharkBooks action.",
            );
        }
        if canonical_ids.len() > 1 {
            let action_ids: Vec<String> = canonical_ids.into_iter().collect();
            return ResolutionOutcome {
                state: ResolutionState::Ambiguous,
                action_id: None,
                execution: ExecutionAvailability::Locked,
                resolved_facts: Vec::new(),
                missing_owner_slots: Vec::new(),
                missing_context_slots: Vec::new(),
                conflicts: Vec::new(),
                attention: Some(make_attention(
                    AttentionReason::AmbiguousAction,
                    ResolutionState::Ambiguous,
                    "More than one SharkBooks action matches.",
                    action_ids.clone(),
                    action_ids,
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                )),
            };
        }

        let action_id = canonical_ids.iter().next().expect("one candidate").clone();
        let action = self
            .registry
            .action(&action_id)
            .expect("canonical candidate is present");
        let execution = self.execution_availability(action);

        if execution != ExecutionAvailability::Executable {
            let (reason, message) = match execution {
                ExecutionAvailability::Locked => (
                    AttentionReason::LockedAction,
                    "This SharkBooks action is known but is not activated on this surface.",
                ),
                ExecutionAvailability::InternalOnly => (
                    AttentionReason::InternalOnly,
                    "This is an internal SharkBooks operation and is not an owner action.",
                ),
                ExecutionAvailability::NotAuthorised => (
                    AttentionReason::NotAuthorised,
                    "This action is outside the authorised SharkBooks capability surface.",
                ),
                ExecutionAvailability::Executable => unreachable!(),
            };
            return self.outcome_with_attention(
                ResolutionState::Known,
                Some(action_id),
                execution,
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                reason,
                message,
            );
        }

        let mut resolved_facts = alias_facts;
        resolved_facts.extend(request.facts.iter().cloned());
        let conflicts = detect_conflicts(&resolved_facts);
        if !conflicts.is_empty() {
            return self.outcome_with_attention(
                ResolutionState::Conflicting,
                Some(action_id),
                execution,
                resolved_facts,
                Vec::new(),
                Vec::new(),
                conflicts,
                AttentionReason::ConflictingInformation,
                "The supplied information conflicts and must be resolved.",
            );
        }

        let supplied_slots: BTreeSet<&str> = resolved_facts
            .iter()
            .map(|fact| fact.slot_id.as_str())
            .collect();
        let mut missing_owner_slots = Vec::new();
        let mut missing_context_slots = Vec::new();
        for slot in &action.required_slots {
            if !slot.required || supplied_slots.contains(slot.slot_id.as_str()) {
                continue;
            }
            match slot.source {
                SlotSource::Owner => missing_owner_slots.push(slot.slot_id.clone()),
                SlotSource::Context => missing_context_slots.push(slot.slot_id.clone()),
            }
        }

        if !missing_owner_slots.is_empty() || !missing_context_slots.is_empty() {
            let reason = if !missing_owner_slots.is_empty() {
                AttentionReason::MissingInformation
            } else {
                AttentionReason::MissingContext
            };
            let message = if !missing_owner_slots.is_empty() {
                "More owner information is required before this action can continue."
            } else {
                "Required application context is not available for this action."
            };
            return self.outcome_with_attention(
                ResolutionState::Unknown,
                Some(action_id),
                execution,
                resolved_facts,
                missing_owner_slots,
                missing_context_slots,
                Vec::new(),
                reason,
                message,
            );
        }

        ResolutionOutcome {
            state: ResolutionState::Known,
            action_id: Some(action_id),
            execution,
            resolved_facts,
            missing_owner_slots: Vec::new(),
            missing_context_slots: Vec::new(),
            conflicts: Vec::new(),
            attention: None,
        }
    }

    pub fn create_confirmation(
        &self,
        action_id: &str,
        state_revision: impl Into<String>,
        facts: &[ActionFact],
        provided: ConfirmationClass,
    ) -> FoundationResult<ConfirmationReceipt> {
        let (canonical, normalized_facts) = self.normalized_action_facts(action_id, facts)?;
        let action = self
            .registry
            .action(&canonical)
            .ok_or_else(|| validation("canonical Action ID is absent"))?;
        if provided < action.confirmation_class {
            return Err(validation(
                "provided confirmation class is lower than the ActionSpec requirement",
            ));
        }
        let state_revision = state_revision.into();
        if state_revision.trim().is_empty() {
            return Err(validation("state revision must not be empty"));
        }
        Ok(ConfirmationReceipt {
            action_id: action.action_id.clone(),
            action_version: action.version,
            state_revision,
            facts_fingerprint: facts_fingerprint(&normalized_facts),
            confirmation_class: provided,
        })
    }

    pub fn validate_confirmation(
        &self,
        receipt: &ConfirmationReceipt,
        current_state_revision: &str,
        current_facts: &[ActionFact],
    ) -> FoundationResult<()> {
        let action = self
            .registry
            .action(&receipt.action_id)
            .ok_or_else(|| stale_confirmation("confirmation Action ID no longer exists"))?;
        if receipt.action_version != action.version
            || receipt.state_revision != current_state_revision
            || receipt.facts_fingerprint != facts_fingerprint(current_facts)
            || receipt.confirmation_class < action.confirmation_class
        {
            return Err(stale_confirmation(
                "confirmation is stale for the current ActionSpec/state/facts",
            ));
        }
        Ok(())
    }

    pub fn prepare_call(
        &self,
        resolution: &ResolutionOutcome,
        operation_id: impl Into<String>,
        invocation_source: InvocationSource,
        state_revision: impl Into<String>,
    ) -> FoundationResult<ActionCall> {
        if resolution.state != ResolutionState::Known
            || resolution.execution != ExecutionAvailability::Executable
            || resolution.attention.is_some()
        {
            return Err(validation(
                "cannot prepare ActionCall from unresolved or locked resolution",
            ));
        }
        let action_id = resolution
            .action_id
            .as_deref()
            .ok_or_else(|| validation("resolved action is missing Action ID"))?;
        let action = self
            .registry
            .action(action_id)
            .ok_or_else(|| validation("resolved Action ID is absent"))?;
        let operation_id = operation_id.into();
        let state_revision = state_revision.into();
        if operation_id.trim().is_empty() || state_revision.trim().is_empty() {
            return Err(validation("operation/state identifiers must not be empty"));
        }
        Ok(ActionCall {
            action_id: action.action_id.clone(),
            action_version: action.version,
            operation_id,
            invocation_source,
            state_revision,
            facts: resolution.resolved_facts.clone(),
        })
    }

    /// Final deterministic pre-execution acceptance gate.
    ///
    /// This method still performs no bookkeeping mutation. It revalidates action
    /// lifecycle/exposure, current state, required confirmation and replay identity.
    /// The operation id is consumed only after every other check succeeds.
    pub fn accept_call(
        &self,
        call: &ActionCall,
        current_state_revision: &str,
        confirmation: Option<&ConfirmationReceipt>,
        replay_guard: &mut ReplayGuard,
    ) -> FoundationResult<()> {
        let action = self
            .registry
            .action(&call.action_id)
            .ok_or_else(|| stale_confirmation("ActionCall Action ID no longer exists"))?;
        if call.action_version != action.version || call.state_revision != current_state_revision {
            return Err(stale_confirmation(
                "ActionCall is stale for the current ActionSpec/state revision",
            ));
        }
        if self.execution_availability(action) != ExecutionAvailability::Executable {
            return Err(validation("ActionCall is no longer exposed/executable"));
        }
        if !detect_conflicts(&call.facts).is_empty() {
            return Err(validation("ActionCall contains conflicting facts"));
        }
        let supplied_slots: BTreeSet<&str> =
            call.facts.iter().map(|fact| fact.slot_id.as_str()).collect();
        if action
            .required_slots
            .iter()
            .any(|slot| slot.required && !supplied_slots.contains(slot.slot_id.as_str()))
        {
            return Err(validation("ActionCall is missing a required slot"));
        }

        if action.confirmation_class != ConfirmationClass::None {
            let receipt = confirmation.ok_or_else(|| validation("ActionCall requires confirmation"))?;
            self.validate_confirmation(receipt, current_state_revision, &call.facts)?;
            if receipt.action_id != call.action_id || receipt.action_version != call.action_version {
                return Err(stale_confirmation(
                    "confirmation does not bind this ActionCall identity",
                ));
            }
        }

        replay_guard.register(call.operation_id.clone())?;
        Ok(())
    }

    fn canonical_candidates_and_alias_facts(
        &self,
        candidates: &[String],
    ) -> (BTreeSet<String>, Vec<ActionFact>) {
        let mut canonical_ids = BTreeSet::new();
        let mut alias_facts = Vec::new();
        for action_id in candidates {
            if let Some(action) = self.registry.action(action_id) {
                canonical_ids.insert(action.action_id.clone());
                continue;
            }
            if let Some(alias) = self.registry.alias(action_id) {
                canonical_ids.insert(alias.target_action_id.clone());
                alias_facts.extend(alias.preset.iter().map(|(slot_id, value)| ActionFact {
                    slot_id: slot_id.clone(),
                    value: value.clone(),
                    source: FactSource::AliasRecipe,
                }));
            }
        }
        (canonical_ids, alias_facts)
    }

    fn normalized_action_facts(
        &self,
        action_id: &str,
        facts: &[ActionFact],
    ) -> FoundationResult<(String, Vec<ActionFact>)> {
        let mut normalized = Vec::new();
        let canonical = if let Some(action) = self.registry.action(action_id) {
            action.action_id.clone()
        } else if let Some(alias) = self.registry.alias(action_id) {
            normalized.extend(alias.preset.iter().map(|(slot_id, value)| ActionFact {
                slot_id: slot_id.clone(),
                value: value.clone(),
                source: FactSource::AliasRecipe,
            }));
            alias.target_action_id.clone()
        } else {
            return Err(validation("unknown Action ID for confirmation"));
        };
        normalized.extend(facts.iter().cloned());
        if !detect_conflicts(&normalized).is_empty() {
            return Err(validation("Action facts conflict with the canonical alias recipe"));
        }
        Ok((canonical, normalized))
    }

    fn execution_availability(&self, action: &ActionSpec) -> ExecutionAvailability {
        match action.backend_state.as_str() {
            "READY" => {
                if self.exposed_action_ids.contains(&action.action_id) {
                    ExecutionAvailability::Executable
                } else {
                    ExecutionAvailability::Locked
                }
            }
            "INTERNAL_ONLY" => ExecutionAvailability::InternalOnly,
            "NOT_AUTHORISED" => ExecutionAvailability::NotAuthorised,
            _ => ExecutionAvailability::Locked,
        }
    }

    fn outcome_with_attention(
        &self,
        state: ResolutionState,
        action_id: Option<String>,
        execution: ExecutionAvailability,
        resolved_facts: Vec<ActionFact>,
        missing_owner_slots: Vec<String>,
        missing_context_slots: Vec<String>,
        conflicts: Vec<ConflictDetail>,
        reason: AttentionReason,
        summary: &str,
    ) -> ResolutionOutcome {
        let action_ids = action_id.clone().into_iter().collect::<Vec<_>>();
        ResolutionOutcome {
            state,
            action_id,
            execution,
            resolved_facts,
            missing_owner_slots: missing_owner_slots.clone(),
            missing_context_slots: missing_context_slots.clone(),
            conflicts: conflicts.clone(),
            attention: Some(make_attention(
                reason,
                state,
                summary,
                action_ids.clone(),
                action_ids,
                missing_owner_slots,
                missing_context_slots,
                conflicts,
            )),
        }
    }
}
