use super::*;

impl Books {
    fn strong_duplicate_row(
        &self,
        strong_identity_key: &str,
    ) -> FoundationResult<Option<(i64, String, String, i64, String)>> {
        let mut stmt = self
            .db
            .conn()
            .prepare(
                "SELECT id, source_account_id, posted_date, signed_amount_minor, currency \
                 FROM shark_bank_activity \
                 WHERE company_slug = ?1 AND strong_identity_key = ?2 LIMIT 1",
            )
            .map_err(sqlite_error)?;
        let mut rows = stmt
            .query((&self.company_slug, strong_identity_key))
            .map_err(sqlite_error)?;
        let Some(row) = rows.next().map_err(sqlite_error)? else {
            return Ok(None);
        };
        Ok(Some((
            row.get(0).map_err(sqlite_error)?,
            row.get(1).map_err(sqlite_error)?,
            row.get(2).map_err(sqlite_error)?,
            row.get(3).map_err(sqlite_error)?,
            row.get(4).map_err(sqlite_error)?,
        )))
    }

    fn file_exact_duplicate_id(
        &self,
        activity: &BankActivityWrite,
    ) -> FoundationResult<Option<i64>> {
        let mut stmt = self
            .db
            .conn()
            .prepare(
                "SELECT id FROM shark_bank_activity \
                 WHERE company_slug = ?1 AND source_file_sha256 = ?2 \
                   AND source_locator = ?3 AND raw_record_sha256 = ?4 LIMIT 1",
            )
            .map_err(sqlite_error)?;
        let mut rows = stmt
            .query((
                &self.company_slug,
                &activity.source_file_sha256,
                &activity.source_locator,
                &activity.raw_record_sha256,
            ))
            .map_err(sqlite_error)?;
        match rows.next().map_err(sqlite_error)? {
            Some(row) => Ok(Some(row.get(0).map_err(sqlite_error)?)),
            None => Ok(None),
        }
    }

    fn insert_bank_activity(&self, activity: &BankActivityWrite) -> FoundationResult<i64> {
        let payload = serde_json::to_string(activity).map_err(|error| {
            validation(format!(
                "could not serialize canonical bank activity: {error}"
            ))
        })?;
        self.db
            .conn()
            .execute(
                "INSERT INTO shark_bank_activity \
                 (company_slug, strong_identity_key, source_file_sha256, source_locator, \
                  raw_record_sha256, source_account_id, posted_date, signed_amount_minor, \
                  currency, payload_json, imported_by) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                (
                    &self.company_slug,
                    activity.strong_identity_key.as_deref(),
                    &activity.source_file_sha256,
                    &activity.source_locator,
                    &activity.raw_record_sha256,
                    &activity.source_account_id,
                    &activity.posted_date,
                    activity.signed_amount_minor,
                    &activity.currency_code,
                    &payload,
                    self.actor.name(),
                ),
            )
            .map_err(sqlite_error)?;
        Ok(self.db.conn().last_insert_rowid())
    }

    pub fn persist_bank_activity_batch(
        &self,
        activities: &[BankActivityWrite],
    ) -> FoundationResult<Vec<BankActivityPersistOutcome>> {
        ensure_application_schema(&self.db)?;
        if activities.is_empty() {
            return Err(validation(
                "bank import confirmation must contain at least one line",
            ));
        }
        if activities.len() > MAX_ACTIVITY_BATCH {
            return Err(validation(
                "bank import confirmation exceeds the 10000-line bound",
            ));
        }
        for activity in activities {
            validate_activity(activity)?;
        }

        self.shark_savepoint("shark_bank_import_confirm", || {
            let mut outcomes = Vec::with_capacity(activities.len());
            for activity in activities {
                if let Some(strong) = activity.strong_identity_key.as_deref() {
                    if let Some((id, source_account, posted_date, amount, currency)) =
                        self.strong_duplicate_row(strong)?
                    {
                        if source_account != activity.source_account_id
                            || posted_date != activity.posted_date
                            || amount != activity.signed_amount_minor
                            || currency != activity.currency_code
                        {
                            return Err(validation(format!(
                                "strong bank identity '{strong}' conflicts with persisted accounting content"
                            )));
                        }
                        outcomes.push(BankActivityPersistOutcome {
                            source_locator: activity.source_locator.clone(),
                            activity_id: id,
                            kind: BankActivityPersistKind::StrongDuplicate,
                        });
                        continue;
                    }
                }
                if let Some(id) = self.file_exact_duplicate_id(activity)? {
                    outcomes.push(BankActivityPersistOutcome {
                        source_locator: activity.source_locator.clone(),
                        activity_id: id,
                        kind: BankActivityPersistKind::FileExactDuplicate,
                    });
                    continue;
                }
                let id = self.insert_bank_activity(activity)?;
                outcomes.push(BankActivityPersistOutcome {
                    source_locator: activity.source_locator.clone(),
                    activity_id: id,
                    kind: BankActivityPersistKind::Created,
                });
            }
            Ok(outcomes)
        })
    }

    fn bank_activity_view_from_parts(
        &self,
        id: i64,
        payload_json: String,
        imported_at: String,
        imported_by: String,
        matched_transaction_id: Option<i64>,
        matched_entry_id: Option<i64>,
        match_level: Option<String>,
        match_score_raw: Option<i64>,
        reasons_json: Option<String>,
        clearance_state: Option<String>,
    ) -> FoundationResult<BankActivityView> {
        if id <= 0 {
            return Err(FoundationError::new(
                FoundationErrorCode::Storage,
                "persisted bank activity has an invalid id",
            ));
        }
        nonblank(&imported_by, "bank activity importer", 100)?;
        let activity: BankActivityWrite = serde_json::from_str(&payload_json).map_err(|error| {
            FoundationError::new(
                FoundationErrorCode::Storage,
                format!("persisted bank activity payload is invalid: {error}"),
            )
        })?;
        validate_activity(&activity)?;

        let any_match = matched_transaction_id.is_some()
            || matched_entry_id.is_some()
            || match_level.is_some()
            || match_score_raw.is_some()
            || reasons_json.is_some()
            || clearance_state.is_some();
        if any_match
            && (matched_transaction_id.is_none()
                || matched_entry_id.is_none()
                || match_level.is_none()
                || match_score_raw.is_none()
                || reasons_json.is_none()
                || clearance_state.is_none())
        {
            return Err(FoundationError::new(
                FoundationErrorCode::Storage,
                "persisted bank match is incomplete",
            ));
        }

        let match_score = match match_score_raw {
            Some(value) => Some(u16::try_from(value).map_err(|_| {
                FoundationError::new(
                    FoundationErrorCode::Storage,
                    "persisted bank match score is outside the supported range",
                )
            })?),
            None => None,
        };
        let match_reasons = match reasons_json {
            Some(value) => serde_json::from_str::<Vec<String>>(&value).map_err(|error| {
                FoundationError::new(
                    FoundationErrorCode::Storage,
                    format!("persisted bank match reasons are invalid: {error}"),
                )
            })?,
            None => Vec::new(),
        };
        if let Some(status) = clearance_state.as_deref() {
            if !matches!(status, "uncleared" | "cleared" | "reconciled") {
                return Err(FoundationError::new(
                    FoundationErrorCode::Storage,
                    "persisted bank match references an invalid clearance state",
                ));
            }
        }

        Ok(BankActivityView {
            id,
            activity,
            imported_at,
            imported_by,
            matched_transaction_id,
            matched_entry_id,
            match_level,
            match_score,
            match_reasons,
            clearance_state,
        })
    }

    pub fn bank_activity(&self, activity_id: i64) -> FoundationResult<BankActivityView> {
        ensure_application_schema(&self.db)?;
        if activity_id <= 0 {
            return Err(validation("bank activity id must be positive"));
        }
        let mut stmt = self
            .db
            .conn()
            .prepare(
                "SELECT a.id, a.payload_json, a.imported_at, a.imported_by, \
                        m.transaction_id, m.entry_id, m.match_level, m.score, m.reasons_json, \
                        e.status \
                 FROM shark_bank_activity a \
                 LEFT JOIN shark_bank_match m \
                   ON m.company_slug = a.company_slug AND m.bank_activity_id = a.id \
                 LEFT JOIN entries e \
                   ON e.id = m.entry_id AND e.transaction_id = m.transaction_id \
                      AND e.company_slug = a.company_slug \
                 WHERE a.company_slug = ?1 AND a.id = ?2",
            )
            .map_err(sqlite_error)?;
        let mut rows = stmt
            .query((&self.company_slug, activity_id))
            .map_err(sqlite_error)?;
        let Some(row) = rows.next().map_err(sqlite_error)? else {
            return Err(FoundationError::new(
                FoundationErrorCode::NotFound,
                format!("bank activity {activity_id} was not found"),
            ));
        };
        self.bank_activity_view_from_parts(
            row.get(0).map_err(sqlite_error)?,
            row.get(1).map_err(sqlite_error)?,
            row.get(2).map_err(sqlite_error)?,
            row.get(3).map_err(sqlite_error)?,
            row.get(4).map_err(sqlite_error)?,
            row.get(5).map_err(sqlite_error)?,
            row.get(6).map_err(sqlite_error)?,
            row.get(7).map_err(sqlite_error)?,
            row.get(8).map_err(sqlite_error)?,
            row.get(9).map_err(sqlite_error)?,
        )
    }

    pub fn list_bank_activity(
        &self,
        limit: i64,
        offset: i64,
    ) -> FoundationResult<Vec<BankActivityView>> {
        ensure_application_schema(&self.db)?;
        if !(1..=MAX_ACTIVITY_PAGE).contains(&limit) {
            return Err(validation("bank activity page limit must be between 1 and 500"));
        }
        if offset < 0 || offset > 1_000_000 {
            return Err(validation("bank activity page offset is outside the supported range"));
        }

        let mut stmt = self
            .db
            .conn()
            .prepare(
                "SELECT a.id, a.payload_json, a.imported_at, a.imported_by, \
                        m.transaction_id, m.entry_id, m.match_level, m.score, m.reasons_json, \
                        e.status \
                 FROM shark_bank_activity a \
                 LEFT JOIN shark_bank_match m \
                   ON m.company_slug = a.company_slug AND m.bank_activity_id = a.id \
                 LEFT JOIN entries e \
                   ON e.id = m.entry_id AND e.transaction_id = m.transaction_id \
                      AND e.company_slug = a.company_slug \
                 WHERE a.company_slug = ?1 \
                 ORDER BY a.posted_date DESC, a.id DESC \
                 LIMIT ?2 OFFSET ?3",
            )
            .map_err(sqlite_error)?;
        let mut rows = stmt
            .query((&self.company_slug, limit, offset))
            .map_err(sqlite_error)?;
        let mut result = Vec::new();
        while let Some(row) = rows.next().map_err(sqlite_error)? {
            result.push(self.bank_activity_view_from_parts(
                row.get(0).map_err(sqlite_error)?,
                row.get(1).map_err(sqlite_error)?,
                row.get(2).map_err(sqlite_error)?,
                row.get(3).map_err(sqlite_error)?,
                row.get(4).map_err(sqlite_error)?,
                row.get(5).map_err(sqlite_error)?,
                row.get(6).map_err(sqlite_error)?,
                row.get(7).map_err(sqlite_error)?,
                row.get(8).map_err(sqlite_error)?,
                row.get(9).map_err(sqlite_error)?,
            )?);
        }
        Ok(result)
    }
}
