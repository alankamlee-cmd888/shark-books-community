use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{FoundationError, FoundationErrorCode, FoundationResult};

const ACTION_REGISTRY_MANIFEST_JSON: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/action_registry_v1_manifest.json"));
const ACTION_REGISTRY_PARTS: &[&str] = &[
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/action_registry_v1_chunk01.jsonl")),
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/action_registry_v1_chunk02.jsonl")),
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/action_registry_v1_chunk03.jsonl")),
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/action_registry_v1_chunk04.jsonl")),
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/action_registry_v1_chunk05.jsonl")),
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/action_registry_v1_chunk06.jsonl")),
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/action_registry_v1_chunk07.jsonl")),
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/action_registry_v1_chunk08.jsonl")),
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/action_registry_v1_chunk09.jsonl")),
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/action_registry_v1_chunk10.jsonl")),
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/action_registry_v1_chunk11.jsonl")),
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/action_registry_v1_chunk12.jsonl")),
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/action_registry_v1_chunk13.jsonl")),
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/action_registry_v1_chunk14.jsonl")),
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/action_registry_v1_chunk15.jsonl")),
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/action_registry_v1_chunk16.jsonl")),
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/action_registry_v1_chunk17.jsonl")),
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/action_registry_v1_chunk18.jsonl")),
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/action_registry_v1_chunk19.jsonl")),
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/action_registry_v1_chunk20.jsonl")),
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/action_registry_v1_chunk21.jsonl")),
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/action_registry_v1_chunk22.jsonl")),
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/action_registry_v1_chunk23.jsonl")),
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/action_registry_v1_chunk24.jsonl")),
];

#[cfg_attr(feature = "action-contract-gen", derive(schemars::JsonSchema, ts_rs::TS))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActionRegistryCounts {
    pub canonical_actions: usize,
    pub voice_eligible_canonical_actions: usize,
    pub aliases: usize,
    pub utterance_fixtures: usize,
}

#[cfg_attr(feature = "action-contract-gen", derive(schemars::JsonSchema, ts_rs::TS))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActionRegistryPolicy {
    pub voice_adds_no_new_authority: bool,
    pub locked_actions_fail_closed: bool,
    pub confirmation_cannot_be_lowered_by_invocation_surface: bool,
    pub manual_text_voice_platform_intent_share_one_action_identity: bool,
}

#[cfg_attr(feature = "action-contract-gen", derive(schemars::JsonSchema, ts_rs::TS))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActionAlias {
    pub alias_action_id: String,
    pub target_action_id: String,
    pub preset: BTreeMap<String, String>,
    pub label: String,
}

#[cfg_attr(feature = "action-contract-gen", derive(schemars::JsonSchema, ts_rs::TS))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ConfirmationClass {
    None,
    Explicit,
    StrongExplicit,
}

#[cfg_attr(feature = "action-contract-gen", derive(schemars::JsonSchema, ts_rs::TS))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SlotSource {
    Owner,
    Context,
}

#[cfg_attr(feature = "action-contract-gen", derive(schemars::JsonSchema, ts_rs::TS))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SlotSpec {
    pub slot_id: String,
    pub label: String,
    pub required: bool,
    pub source: SlotSource,
    #[serde(default)]
    pub choices: Vec<String>,
}

#[cfg_attr(feature = "action-contract-gen", derive(schemars::JsonSchema, ts_rs::TS))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActionSpec {
    pub action_id: String,
    pub version: u32,
    pub family: String,
    pub manual_label: String,
    pub owner_intent: String,
    pub implementation_state: String,
    pub availability_state: String,
    pub evidence_state: Option<String>,
    pub authority_class: String,
    pub confirmation_policy: String,
    pub confirmation_class: ConfirmationClass,
    pub voice_parity: String,
    pub voice_eligible: bool,
    pub backend_basis: String,
    pub authority_source: String,
    pub hard_boundary: Option<String>,
    pub required_slots: Vec<SlotSpec>,
    #[serde(default)]
    pub choice_hints: Vec<String>,
    #[serde(default)]
    pub utterances: Vec<String>,
    pub backend_state: String,
}

#[cfg_attr(feature = "action-contract-gen", derive(schemars::JsonSchema, ts_rs::TS))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActionRegistryDocument {
    pub schema: String,
    pub date: String,
    pub source_authorities: Vec<String>,
    pub counts: ActionRegistryCounts,
    pub policy: ActionRegistryPolicy,
    pub aliases: Vec<ActionAlias>,
    pub actions: Vec<ActionSpec>,
}

impl ActionRegistryDocument {
    pub fn action(&self, action_id: &str) -> Option<&ActionSpec> {
        self.actions.iter().find(|action| action.action_id == action_id)
    }

    pub fn alias(&self, action_id: &str) -> Option<&ActionAlias> {
        self.aliases
            .iter()
            .find(|alias| alias.alias_action_id == action_id)
    }

    pub fn canonical_action_id(&self, action_id: &str) -> Option<&str> {
        if let Some(action) = self.action(action_id) {
            return Some(action.action_id.as_str());
        }
        self.alias(action_id)
            .and_then(|alias| self.action(&alias.target_action_id))
            .map(|action| action.action_id.as_str())
    }

    pub fn validate(&self) -> FoundationResult<()> {
        if self.schema != "sharkbooks-action-registry-v1" {
            return Err(validation("unexpected Action Registry schema"));
        }
        if self.counts.canonical_actions != 206
            || self.counts.voice_eligible_canonical_actions != 175
            || self.counts.aliases != 5
            || self.counts.utterance_fixtures != 700
        {
            return Err(validation("Action Registry frozen counts do not match authority"));
        }
        if self.actions.len() != self.counts.canonical_actions
            || self.aliases.len() != self.counts.aliases
        {
            return Err(validation("Action Registry collection lengths do not match counts"));
        }

        let mut canonical_ids = BTreeSet::new();
        let mut voice_count = 0usize;
        let mut utterance_count = 0usize;
        let mut required_when_exposed = 0usize;
        let mut required_when_activated = 0usize;

        for action in &self.actions {
            if action.action_id.trim().is_empty() || action.version == 0 {
                return Err(validation("Action Registry contains invalid identity"));
            }
            if !canonical_ids.insert(action.action_id.as_str()) {
                return Err(validation("Action Registry contains duplicate canonical Action ID"));
            }
            if action.voice_eligible {
                voice_count += 1;
                if action.utterances.len() < 4 {
                    return Err(validation(
                        "voice-eligible canonical action has fewer than four fixtures",
                    ));
                }
                utterance_count += action.utterances.len();
            } else if !action.utterances.is_empty() {
                return Err(validation(
                    "non-voice canonical action unexpectedly carries utterance fixtures",
                ));
            }

            match (action.voice_eligible, action.voice_parity.as_str()) {
                (true, "REQUIRED_WHEN_EXPOSED") => required_when_exposed += 1,
                (true, "REQUIRED_WHEN_ACTIVATED") => required_when_activated += 1,
                (false, "NOT_APPLICABLE_INTERNAL") | (false, "NO") => {}
                _ => {
                    return Err(validation(
                        "voice parity is inconsistent with voice eligibility",
                    ));
                }
            }
        }

        if voice_count != 175
            || utterance_count != 700
            || required_when_exposed + required_when_activated != voice_count
        {
            return Err(validation("Action Registry voice totals do not match authority"));
        }

        let mut alias_ids = BTreeSet::new();
        for alias in &self.aliases {
            if canonical_ids.contains(alias.alias_action_id.as_str()) {
                return Err(validation("alias must not duplicate a canonical Action ID"));
            }
            if !alias_ids.insert(alias.alias_action_id.as_str()) {
                return Err(validation("duplicate Action alias"));
            }
            if !canonical_ids.contains(alias.target_action_id.as_str()) {
                return Err(validation("Action alias target is not canonical"));
            }
        }

        for prohibited in [
            "RECORD.DELETE",
            "INVENTORY.COGS",
            "INVENTORY.FULL_VALUATION",
            "AI.AUTONOMOUS_ACCOUNTING",
            "CIS.PROCESS",
            "COMPANY.PARTNERSHIP_ACCOUNTING",
            "HMRC.DIRECT_SUBMIT",
            "OPEN_BANKING.LIVE_FEEDS",
            "PAYROLL.PROCESS",
            "VAT.ACCOUNTING_FILING",
        ] {
            let Some(action) = self.action(prohibited) else {
                return Err(validation("required prohibited Action ID is absent"));
            };
            if action.voice_eligible || action.backend_state == "READY" {
                return Err(validation(
                    "prohibited Action ID must remain outside voice/backend execution",
                ));
            }
        }

        Ok(())
    }
}

pub fn load_action_registry() -> FoundationResult<ActionRegistryDocument> {
    #[derive(Deserialize)]
    struct RegistryManifest {
        schema: String,
        date: String,
        source_authorities: Vec<String>,
        counts: ActionRegistryCounts,
        policy: ActionRegistryPolicy,
        aliases: Vec<ActionAlias>,
    }

    let manifest: RegistryManifest = serde_json::from_str(ACTION_REGISTRY_MANIFEST_JSON)
        .map_err(|error| validation(format!("invalid action registry manifest: {error}")))?;
    let mut actions = Vec::with_capacity(manifest.counts.canonical_actions);
    for (part_index, part) in ACTION_REGISTRY_PARTS.iter().enumerate() {
        for (line_index, line) in part.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let action: ActionSpec = serde_json::from_str(line).map_err(|error| {
                validation(format!(
                    "invalid action registry part {} line {}: {error}",
                    part_index + 1,
                    line_index + 1
                ))
            })?;
            actions.push(action);
        }
    }
    let registry = ActionRegistryDocument {
        schema: manifest.schema,
        date: manifest.date,
        source_authorities: manifest.source_authorities,
        counts: manifest.counts,
        policy: manifest.policy,
        aliases: manifest.aliases,
        actions,
    };
    registry.validate()?;
    Ok(registry)
}
