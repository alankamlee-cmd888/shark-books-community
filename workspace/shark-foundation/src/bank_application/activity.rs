use super::*;

impl Books {
    fn shark_savepoint<T>(
        &self,
        name: &'static str,
        operation: impl FnOnce() -> FoundationResult<T>,
    ) -> FoundationResult<T> {
        self.db
            .conn()
            .execute_batch(&format!("SAVEPOINT {name}"))
            .map_err(sqlite_error)?;
        match operation() {
            Ok(value) => {
                self.db
                    .conn()
                    .execute_batch(&format!("RELEASE {name}"))
                    .map_err(sqlite_error)?;
                Ok(value)
            }
            Err(error) => {
                let _ = self
                    .db
                    .conn()
                    .execute_batch(&format!("ROLLBACK TO {name}; RELEASE {name}"));
                Err(error)
            }
        }
    }

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

    fn file_exact_duplicate_id(&self, activity: &BankActivityWrite) -> FoundationResult<Option<i64>> {
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
        let payload = serde_json::to_string(activity)
            .map_err(|error| validation(format!("could not serialize canonical bank activity: {error}")))?;
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
            return Err(validation("bank import confirmation must contain at least one line"));
        }
        if activities.len() > MAX_ACTIVITY_BATCH {
            return Err(validation("bank import confirmation exceeds the 10000-line bound"));
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
}
