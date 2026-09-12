use super::*;
use std::collections::HashSet;

impl Books {
    fn match_view_from_values(
        &self,
        bank_activity_id: i64,
        transaction_id: i64,
        entry_id: i64,
        match_level: String,
        score_raw: i64,
        reasons_json: String,
        confirmed_by: String,
        confirmed_at: String,
    ) -> FoundationResult<BankMatchView> {
        let score = u16::try_from(score_raw).map_err(|_| {
            FoundationError::new(
                FoundationErrorCode::Storage,
                "persisted bank match score is outside the supported range",
            )
        })?;
        let reasons = serde_json::from_str::<Vec<String>>(&reasons_json).map_err(|error| {
            FoundationError::new(
                FoundationErrorCode::Storage,
                format!("persisted bank match reasons are invalid: {error}"),
            )
        })?;
        Ok(BankMatchView {
            bank_activity_id,
            transaction_id,
            entry_id,
            match_level,
            score,
            reasons,
            confirmed_by,
            confirmed_at,
        })
    }

    fn query_bank_match_for_activity(
        &self,
        bank_activity_id: i64,
    ) -> FoundationResult<Option<BankMatchView>> {
        let mut stmt = self
            .db
            .conn()
            .prepare(
                "SELECT bank_activity_id, transaction_id, entry_id, match_level, score, \
                        reasons_json, confirmed_by, confirmed_at \
                 FROM shark_bank_match \
                 WHERE company_slug = ?1 AND bank_activity_id = ?2 LIMIT 1",
            )
            .map_err(sqlite_error)?;
        let mut rows = stmt
            .query((&self.company_slug, bank_activity_id))
            .map_err(sqlite_error)?;
        let Some(row) = rows.next().map_err(sqlite_error)? else {
            return Ok(None);
        };
        Ok(Some(self.match_view_from_values(
            row.get(0).map_err(sqlite_error)?,
            row.get(1).map_err(sqlite_error)?,
            row.get(2).map_err(sqlite_error)?,
            row.get(3).map_err(sqlite_error)?,
            row.get(4).map_err(sqlite_error)?,
            row.get(5).map_err(sqlite_error)?,
            row.get(6).map_err(sqlite_error)?,
            row.get(7).map_err(sqlite_error)?,
        )?))
    }

    fn query_bank_match_for_entry(
        &self,
        transaction_id: i64,
        entry_id: i64,
    ) -> FoundationResult<Option<BankMatchView>> {
        let mut stmt = self
            .db
            .conn()
            .prepare(
                "SELECT bank_activity_id, transaction_id, entry_id, match_level, score, \
                        reasons_json, confirmed_by, confirmed_at \
                 FROM shark_bank_match \
                 WHERE company_slug = ?1 AND transaction_id = ?2 AND entry_id = ?3 LIMIT 1",
            )
            .map_err(sqlite_error)?;
        let mut rows = stmt
            .query((&self.company_slug, transaction_id, entry_id))
            .map_err(sqlite_error)?;
        let Some(row) = rows.next().map_err(sqlite_error)? else {
            return Ok(None);
        };
        Ok(Some(self.match_view_from_values(
            row.get(0).map_err(sqlite_error)?,
            row.get(1).map_err(sqlite_error)?,
            row.get(2).map_err(sqlite_error)?,
            row.get(3).map_err(sqlite_error)?,
            row.get(4).map_err(sqlite_error)?,
            row.get(5).map_err(sqlite_error)?,
            row.get(6).map_err(sqlite_error)?,
            row.get(7).map_err(sqlite_error)?,
        )?))
    }

    pub fn bank_match_for_activity(
        &self,
        bank_activity_id: i64,
    ) -> FoundationResult<Option<BankMatchView>> {
        ensure_application_schema(&self.db)?;
        if bank_activity_id <= 0 {
            return Err(validation("bank activity id must be positive"));
        }
        self.query_bank_match_for_activity(bank_activity_id)
    }

    pub fn bank_match_for_entry(
        &self,
        transaction_id: i64,
        entry_id: i64,
    ) -> FoundationResult<Option<BankMatchView>> {
        ensure_application_schema(&self.db)?;
        if transaction_id <= 0 || entry_id <= 0 {
            return Err(validation("bank match transaction and entry ids must be positive"));
        }
        self.query_bank_match_for_entry(transaction_id, entry_id)
    }

    fn validate_match_write(&self, write: &BankMatchWrite) -> FoundationResult<()> {
        if write.bank_activity_id <= 0 || write.transaction_id <= 0 || write.entry_id <= 0 {
            return Err(validation("bank match ids must be positive"));
        }
        if !matches!(write.match_level.as_str(), "possible" | "likely" | "exact") {
            return Err(validation(
                "bank match level must be possible, likely or exact",
            ));
        }
        if write.score == 0 {
            return Err(validation("confirmed bank match score must be positive"));
        }
        if write.reasons.is_empty() || write.reasons.len() > 32 {
            return Err(validation(
                "confirmed bank match must contain between 1 and 32 reasons",
            ));
        }
        let mut unique = HashSet::new();
        for reason in &write.reasons {
            nonblank(reason, "bank match reason", 64)?;
            if !unique.insert(reason.as_str()) {
                return Err(validation("confirmed bank match reasons must be unique"));
            }
        }
        Ok(())
    }

    fn exact_business_bank_entry(
        &self,
        transaction_id: i64,
        entry_id: i64,
    ) -> FoundationResult<(String, i64)> {
        let (_transaction, entries) = db::get_transaction(
            self.db.conn(),
            &self.company_slug,
            transaction_id,
        )
        .map_err(map_cli_error)?;
        let mut bank_entries = entries
            .iter()
            .filter(|entry| entry.account_code == BUSINESS_BANK_ACCOUNT_CODE);
        let bank = bank_entries.next().ok_or_else(|| {
            validation("candidate transaction has no Business Bank entry")
        })?;
        if bank_entries.next().is_some() {
            return Err(validation(
                "candidate transaction has more than one Business Bank entry",
            ));
        }
        if bank.id != entry_id {
            return Err(validation(
                "confirmed bank entry does not match the authoritative Business Bank entry",
            ));
        }
        let signed = signed_bank_amount(&bank.direction, bank.amount)?;
        Ok((bank.status.clone(), signed))
    }

    pub fn confirm_bank_match(
        &self,
        write: &BankMatchWrite,
    ) -> FoundationResult<BankMatchPersistOutcome> {
        ensure_application_schema(&self.db)?;
        self.validate_match_write(write)?;
        let _ = self.bank_activity(write.bank_activity_id)?;

        if let Some(existing) = self.query_bank_match_for_activity(write.bank_activity_id)? {
            if existing.transaction_id == write.transaction_id
                && existing.entry_id == write.entry_id
                && existing.match_level == write.match_level
                && existing.score == write.score
                && existing.reasons == write.reasons
            {
                return Ok(BankMatchPersistOutcome::AlreadyConfirmed(existing));
            }
            return Err(validation(
                "bank activity already has a different confirmed match",
            ));
        }
        if let Some(existing) =
            self.query_bank_match_for_entry(write.transaction_id, write.entry_id)?
        {
            if existing.bank_activity_id != write.bank_activity_id {
                return Err(validation(
                    "Business Bank entry is already matched to different bank activity",
                ));
            }
        }

        self.shark_savepoint("shark_bank_match_confirm", || {
            let (status, _signed_amount) =
                self.exact_business_bank_entry(write.transaction_id, write.entry_id)?;
            if status != "uncleared" {
                return Err(validation(
                    "bank match confirmation requires an uncleared Business Bank entry",
                ));
            }
            if self
                .query_bank_match_for_activity(write.bank_activity_id)?
                .is_some()
                || self
                    .query_bank_match_for_entry(write.transaction_id, write.entry_id)?
                    .is_some()
            {
                return Err(validation(
                    "bank match state changed before confirmation; review again",
                ));
            }

            let prior = db::set_entry_status(
                self.db.conn(),
                &self.actor,
                &self.company_slug,
                write.transaction_id,
                write.entry_id,
                EntryStatus::Cleared,
            )
            .map_err(map_cli_error)?;
            if prior != EntryStatus::Uncleared {
                return Err(validation(
                    "bank match attempted an illegal clearance transition",
                ));
            }

            let reasons_json = serde_json::to_string(&write.reasons).map_err(|error| {
                validation(format!("could not serialize bank match reasons: {error}"))
            })?;
            self.db
                .conn()
                .execute(
                    "INSERT INTO shark_bank_match \
                     (company_slug, bank_activity_id, transaction_id, entry_id, match_level, \
                      score, reasons_json, confirmed_by) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    (
                        &self.company_slug,
                        write.bank_activity_id,
                        write.transaction_id,
                        write.entry_id,
                        &write.match_level,
                        i64::from(write.score),
                        &reasons_json,
                        self.actor.name(),
                    ),
                )
                .map_err(sqlite_error)?;
            let confirmed = self
                .query_bank_match_for_activity(write.bank_activity_id)?
                .ok_or_else(|| {
                    FoundationError::new(
                        FoundationErrorCode::Storage,
                        "confirmed bank match could not be re-read",
                    )
                })?;
            Ok(BankMatchPersistOutcome::Confirmed(confirmed))
        })
    }

    fn reconciliation_record_unchecked(
        &self,
        statement_id: &str,
    ) -> FoundationResult<Option<BankReconciliationRecord>> {
        let mut stmt = self
            .db
            .conn()
            .prepare(
                "SELECT id, statement_id, statement_date, opening_balance_minor, \
                        ending_balance_minor, finalized_by, finalized_at \
                 FROM shark_bank_reconciliation \
                 WHERE company_slug = ?1 AND statement_id = ?2 LIMIT 1",
            )
            .map_err(sqlite_error)?;
        let mut rows = stmt
            .query((&self.company_slug, statement_id))
            .map_err(sqlite_error)?;
        let Some(row) = rows.next().map_err(sqlite_error)? else {
            return Ok(None);
        };
        let reconciliation_id: i64 = row.get(0).map_err(sqlite_error)?;
        let stored_statement_id: String = row.get(1).map_err(sqlite_error)?;
        let statement_date: String = row.get(2).map_err(sqlite_error)?;
        let opening_balance_minor: i64 = row.get(3).map_err(sqlite_error)?;
        let ending_balance_minor: i64 = row.get(4).map_err(sqlite_error)?;
        let finalized_by: String = row.get(5).map_err(sqlite_error)?;
        let finalized_at: String = row.get(6).map_err(sqlite_error)?;
        drop(rows);
        drop(stmt);

        let mut member_stmt = self
            .db
            .conn()
            .prepare(
                "SELECT transaction_id, entry_id, signed_amount_minor \
                 FROM shark_bank_reconciliation_entry \
                 WHERE reconciliation_id = ?1 AND company_slug = ?2 \
                 ORDER BY entry_id",
            )
            .map_err(sqlite_error)?;
        let mut member_rows = member_stmt
            .query((reconciliation_id, &self.company_slug))
            .map_err(sqlite_error)?;
        let mut entries = Vec::new();
        while let Some(member) = member_rows.next().map_err(sqlite_error)? {
            entries.push(BankReconciliationEntryWrite {
                transaction_id: member.get(0).map_err(sqlite_error)?,
                entry_id: member.get(1).map_err(sqlite_error)?,
                signed_amount_minor: member.get(2).map_err(sqlite_error)?,
            });
        }
        let view = BankReconciliationView {
            statement_id: stored_statement_id,
            statement_date,
            opening_balance_minor,
            ending_balance_minor,
            entry_count: entries.len(),
            finalized_by,
            finalized_at,
        };
        Ok(Some(BankReconciliationRecord {
            reconciliation: view,
            entries,
        }))
    }

    pub fn bank_reconciliation(
        &self,
        statement_id: &str,
    ) -> FoundationResult<Option<BankReconciliationRecord>> {
        ensure_application_schema(&self.db)?;
        nonblank(statement_id, "statement id", 128)?;
        self.reconciliation_record_unchecked(statement_id)
    }

    fn reconciliation_entries_equal(
        left: &[BankReconciliationEntryWrite],
        right: &[BankReconciliationEntryWrite],
    ) -> bool {
        let mut left_rows: Vec<_> = left
            .iter()
            .map(|entry| {
                (
                    entry.transaction_id,
                    entry.entry_id,
                    entry.signed_amount_minor,
                )
            })
            .collect();
        let mut right_rows: Vec<_> = right
            .iter()
            .map(|entry| {
                (
                    entry.transaction_id,
                    entry.entry_id,
                    entry.signed_amount_minor,
                )
            })
            .collect();
        left_rows.sort_unstable();
        right_rows.sort_unstable();
        left_rows == right_rows
    }

    fn reconciliation_is_identical(
        existing: &BankReconciliationRecord,
        write: &BankReconciliationWrite,
    ) -> bool {
        existing.reconciliation.statement_id == write.statement_id
            && existing.reconciliation.statement_date == write.statement_date
            && existing.reconciliation.opening_balance_minor == write.opening_balance_minor
            && existing.reconciliation.ending_balance_minor == write.ending_balance_minor
            && Self::reconciliation_entries_equal(&existing.entries, &write.entries)
    }

    fn validate_reconciliation_write(
        &self,
        write: &BankReconciliationWrite,
    ) -> FoundationResult<()> {
        nonblank(&write.statement_id, "statement id", 128)?;
        iso_date(&write.statement_date, "statement date")?;
        if write.entries.is_empty() {
            return Err(validation(
                "reconciliation finalisation requires at least one selected entry",
            ));
        }
        if write.entries.len() > MAX_RECONCILIATION_ENTRIES {
            return Err(validation(
                "reconciliation finalisation exceeds the 1000-entry bound",
            ));
        }
        let mut transaction_ids = HashSet::new();
        let mut entry_ids = HashSet::new();
        for entry in &write.entries {
            if entry.transaction_id <= 0 || entry.entry_id <= 0 {
                return Err(validation(
                    "reconciliation transaction and entry ids must be positive",
                ));
            }
            if entry.signed_amount_minor == 0 {
                return Err(validation(
                    "reconciliation entry amount must be non-zero whole pence",
                ));
            }
            if !transaction_ids.insert(entry.transaction_id) {
                return Err(validation(
                    "reconciliation transaction ids must be unique",
                ));
            }
            if !entry_ids.insert(entry.entry_id) {
                return Err(validation("reconciliation entry ids must be unique"));
            }
        }
        Ok(())
    }

    pub fn finalize_bank_reconciliation(
        &self,
        write: &BankReconciliationWrite,
    ) -> FoundationResult<BankReconciliationPersistOutcome> {
        ensure_application_schema(&self.db)?;
        self.validate_reconciliation_write(write)?;

        if let Some(existing) = self.reconciliation_record_unchecked(&write.statement_id)? {
            if Self::reconciliation_is_identical(&existing, write) {
                return Ok(BankReconciliationPersistOutcome::AlreadyFinalized(
                    existing.reconciliation,
                ));
            }
            return Err(validation(
                "statement id is already finalized with different reconciliation content",
            ));
        }

        self.shark_savepoint("shark_bank_reconcile_finalise", || {
            if self
                .reconciliation_record_unchecked(&write.statement_id)?
                .is_some()
            {
                return Err(validation(
                    "reconciliation state changed before finalisation; review again",
                ));
            }

            let mut resolved = Vec::with_capacity(write.entries.len());
            let mut computed = i128::from(write.opening_balance_minor);
            for requested in &write.entries {
                let (status, signed_amount) =
                    self.exact_business_bank_entry(requested.transaction_id, requested.entry_id)?;
                if status != "cleared" {
                    return Err(validation(
                        "reconciliation finalisation requires every Business Bank entry to still be cleared",
                    ));
                }
                if signed_amount != requested.signed_amount_minor {
                    return Err(validation(
                        "reconciliation entry amount changed after preview",
                    ));
                }
                let confirmed = self
                    .query_bank_match_for_entry(requested.transaction_id, requested.entry_id)?
                    .ok_or_else(|| {
                        validation(
                            "reconciliation entry has no owner-confirmed bank match",
                        )
                    })?;
                computed = computed
                    .checked_add(i128::from(signed_amount))
                    .ok_or_else(|| validation("reconciliation balance arithmetic overflow"))?;
                resolved.push((requested.clone(), confirmed.bank_activity_id));
            }
            if computed != i128::from(write.ending_balance_minor) {
                return Err(validation(format!(
                    "statement difference must be zero; difference={} pence",
                    i128::from(write.ending_balance_minor) - computed
                )));
            }

            self.db
                .conn()
                .execute(
                    "INSERT INTO shark_bank_reconciliation \
                     (company_slug, statement_id, statement_date, opening_balance_minor, \
                      ending_balance_minor, finalized_by) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    (
                        &self.company_slug,
                        &write.statement_id,
                        &write.statement_date,
                        write.opening_balance_minor,
                        write.ending_balance_minor,
                        self.actor.name(),
                    ),
                )
                .map_err(sqlite_error)?;
            let reconciliation_id = self.db.conn().last_insert_rowid();

            for (requested, bank_activity_id) in &resolved {
                self.db
                    .conn()
                    .execute(
                        "INSERT INTO shark_bank_reconciliation_entry \
                         (reconciliation_id, company_slug, bank_activity_id, transaction_id, \
                          entry_id, signed_amount_minor) \
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                        (
                            reconciliation_id,
                            &self.company_slug,
                            *bank_activity_id,
                            requested.transaction_id,
                            requested.entry_id,
                            requested.signed_amount_minor,
                        ),
                    )
                    .map_err(sqlite_error)?;

                let prior = db::set_entry_status(
                    self.db.conn(),
                    &self.actor,
                    &self.company_slug,
                    requested.transaction_id,
                    requested.entry_id,
                    EntryStatus::Reconciled,
                )
                .map_err(map_cli_error)?;
                if prior != EntryStatus::Cleared {
                    return Err(validation(
                        "reconciliation attempted an illegal clearance transition",
                    ));
                }
            }
            Ok(())
        })?;

        let record = self
            .reconciliation_record_unchecked(&write.statement_id)?
            .ok_or_else(|| {
                FoundationError::new(
                    FoundationErrorCode::Storage,
                    "finalized reconciliation could not be re-read",
                )
            })?;
        Ok(BankReconciliationPersistOutcome::Finalized(
            record.reconciliation,
        ))
    }
}
