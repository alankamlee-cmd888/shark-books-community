use shark_books_core::{
    bank_import::{preview_csv, CsvAmountMapping, CsvDateFormat, CsvMappingProfile},
    documents::{DocumentReference, IntegrityStatus, ReadVerifiedDocumentRequest, StorageCustody, StorageProvider, StorageRootDescriptor},
    matching::{confirm_match, finalize_reconciliation, rank_matches, ClearanceState, LedgerCandidate, MatchLevel, ReconciliationEntry},
    plan_expense, BusinessUse, ExpenseCategory, ExpenseRecord, GbpAmount, RecordId,
    SettlementAccount, SourceKind,
};

fn id(value: &str) -> RecordId {
    RecordId::new(value).expect("valid test id")
}

#[test]
fn sbc2_to_sbc5_end_to_end_contract_is_coherent() {
    // SBC-3: a source file becomes an immutable canonical bank-line preview
    // with exact pence and source provenance before any ledger action.
    let profile = CsvMappingProfile::new(
        id("profile-1"),
        "Stage F test profile",
        ',',
        "Date",
        None,
        "Description",
        Some("Payee".into()),
        Some("Reference".into()),
        Some("Transaction ID".into()),
        Some("Currency".into()),
        CsvAmountMapping::Signed { amount_header: "Amount".into() },
        CsvDateFormat::IsoYmd,
    )
    .expect("valid CSV mapping profile");

    let csv = concat!(
        "Date,Description,Payee,Reference,Transaction ID,Currency,Amount\n",
        "2026-09-09,Office supplies,Stationers,BANK-001,TXN-001,GBP,-47.23\n"
    );
    let preview = preview_csv(csv, id("bank-1"), &profile).expect("CSV preview");
    assert!(preview.can_commit());
    assert!(preview.errors().is_empty());
    assert_eq!(preview.lines().len(), 1);
    let bank_line = &preview.lines()[0];
    assert_eq!(bank_line.signed_amount_minor(), -4_723);
    assert_eq!(bank_line.provenance().kind(), SourceKind::Csv);
    assert!(bank_line.strong_identity_key().is_some());

    // SBC-2: the imported fact can feed a deterministic Shark-owned domain
    // object and balanced posting plan. The plan is the application-layer
    // boundary consumed later by the already-frozen Shark facade.
    let expense = ExpenseRecord::new(
        id("expense-1"),
        "Office supplies",
        bank_line.posted_date(),
        GbpAmount::positive_minor(4_723).expect("positive GBP pence"),
        ExpenseCategory::OfficePhoneSoftware,
        BusinessUse::Business,
        SettlementAccount::BusinessBank,
        None,
        bank_line.provenance().clone(),
    )
    .expect("valid expense");
    let posting_plan = plan_expense(&expense).expect("posting plan");
    assert!(posting_plan.is_balanced());
    assert_eq!(posting_plan.total_debits(), 4_723);
    assert_eq!(posting_plan.total_credits(), 4_723);

    // SBC-4: matching is deterministic but still user-confirmed. Matching and
    // duplicate identity stay separate, and confirmation moves only to Cleared.
    let ledger_candidate = LedgerCandidate::new(
        id("expense-1"),
        id("bank-1"),
        bank_line.posted_date(),
        -4_723,
        "Office supplies",
        Some("Stationers".into()),
        Some("BANK-001".into()),
        bank_line.strong_identity_key(),
        ClearanceState::Uncleared,
    )
    .expect("ledger candidate");
    let matches = rank_matches(bank_line, std::slice::from_ref(&ledger_candidate));
    assert!(!matches.ambiguous_top());
    assert!(matches.requires_user_confirmation());
    assert_eq!(matches.recommended_candidate_id(), Some(ledger_candidate.id()));
    let assessment = &matches.candidates()[0];
    assert_eq!(assessment.level(), MatchLevel::Exact);
    assert!(assessment.requires_user_confirmation());

    let cleared = confirm_match(&ledger_candidate, assessment, "stage-f-user")
        .expect("user-confirmed match");
    assert_eq!(cleared.from(), ClearanceState::Uncleared);
    assert_eq!(cleared.to(), ClearanceState::Cleared);

    // Statement reconciliation is a distinct step and may complete only when
    // exact integer-pence balance and cleared-state prerequisites are satisfied.
    let entry = ReconciliationEntry::new(id("expense-1"), -4_723, ClearanceState::Cleared)
        .expect("reconciliation entry");
    let reconciliation = finalize_reconciliation(
        id("statement-2026-09"),
        bank_line.posted_date(),
        100_000,
        95_277,
        &[entry],
        "stage-f-user",
    )
    .expect("zero-difference reconciliation");
    assert!(reconciliation.check().can_finalize());
    assert_eq!(reconciliation.check().difference_minor(), 0);
    assert_eq!(reconciliation.transitions().len(), 1);
    assert_eq!(reconciliation.transitions()[0].from(), ClearanceState::Cleared);
    assert_eq!(reconciliation.transitions()[0].to(), ClearanceState::Reconciled);

    // SBC-5: documentary evidence is attached to the bookkeeping record while
    // custody remains with the user. Integrity metadata survives the boundary
    // and produces a bounded verified-read request rather than an arbitrary path.
    let root = StorageRootDescriptor::new(
        id("documents-root"),
        StorageProvider::LocalFilesystem,
        "My SharkBooks documents",
    )
    .expect("user-controlled storage root");
    assert_eq!(root.custody(), StorageCustody::UserControlled);

    let bytes = b"Office supplies receipt - GBP 47.23\n";
    let document = DocumentReference::from_bytes(
        id("document-1"),
        ledger_candidate.id().clone(),
        &root,
        "receipts/2026/office-supplies.txt",
        "office-supplies.txt",
        Some("text/plain".into()),
        bytes,
    )
    .expect("document reference");
    assert_eq!(document.attached_record_id(), ledger_candidate.id());
    assert_eq!(document.verify_bytes(bytes), IntegrityStatus::Verified);

    let read_request = ReadVerifiedDocumentRequest::for_reference(&document);
    assert_eq!(read_request.storage_root_id(), root.id());
    assert_eq!(read_request.relative_path(), document.relative_path());
    assert_eq!(read_request.expected_sha256(), document.sha256());
    assert_eq!(read_request.expected_byte_len(), document.byte_len());
}
