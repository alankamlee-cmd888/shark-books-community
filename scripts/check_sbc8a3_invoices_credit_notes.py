from __future__ import annotations
import pathlib, subprocess, sys, json, re

ROOT = pathlib.Path(__file__).resolve().parents[1]
ALLOWED = {
    "product/shark-books-core/src/lib.rs",
    "workspace/shark-foundation/src/lib.rs",
    "workspace/shark-foundation/src/invoice_application.rs",
    "workspace/shark-foundation/src/quote_application.rs",
    "workspace/shark-foundation/src/bank_application/mod.rs",
    "workspace/shark-tauri-spike/src/owner_supporting_data.rs",
    "workspace/ui/src/App.vue",
    "workspace/ui/src/lib/tauri.ts",
    "workspace/ui/src/screens/InvoicesScreen.vue",
    "workspace/ui/src/components/InvoicesTable.vue",
    "scripts/check_sbc8a3_invoices_credit_notes.py",
}
REQUIRED_FILES = [
    "workspace/shark-foundation/src/invoice_application.rs",
    "workspace/ui/src/screens/InvoicesScreen.vue",
    "workspace/ui/src/components/InvoicesTable.vue",
]
REQUIRED_TOKENS = {
    "workspace/shark-foundation/src/invoice_application.rs": [
        "InvoiceState", "CreditNoteState", "draft", "issued", "part_paid", "paid", "cancelled",
        "remaining", "manual", "credit", "quote",
    ],
    "workspace/shark-tauri-spike/src/owner_supporting_data.rs": [
        "CreateInvoiceDraft", "IssueInvoice", "RecordManualPayment", "CreateCreditNote",
    ],
    "workspace/ui/src/lib/tauri.ts": [
        "InvoiceState", "CreditNoteState", "Invoice", "CreditNote",
    ],
    "workspace/ui/src/screens/InvoicesScreen.vue": ["invoice"],
}
FORBIDDEN_ADDITIONS = [
    r"open.?banking", r"payment.?provider", r"stripe", r"paypal", r"vat.?return",
    r"cis.?return", r"payroll", r"hmrc.?submit", r"pdf.?renderer",
]
errors=[]
for rel in REQUIRED_FILES:
    if not (ROOT/rel).is_file():
        errors.append(f"missing required file: {rel}")

p=subprocess.run(["git","status","--porcelain"],cwd=ROOT,text=True,capture_output=True)
if p.returncode:
    errors.append("git status failed: "+p.stderr.strip())
else:
    for line in p.stdout.splitlines():
        if len(line)<4: continue
        rel=line[3:].replace("\\","/")
        if rel.startswith(("product/shark-books-core/target/","workspace/target/","upstream/")):
            continue
        if rel not in ALLOWED:
            errors.append(f"out-of-allowlist change: {rel}")

for rel,tokens in REQUIRED_TOKENS.items():
    path=ROOT/rel
    if not path.is_file():
        continue
    text=path.read_text(encoding="utf-8",errors="replace")
    low=text.lower()
    for token in tokens:
        if token.lower() not in low:
            errors.append(f"missing token {token!r} in {rel}")

p=subprocess.run(["git","diff","--unified=0"],cwd=ROOT,text=True,capture_output=True,errors="replace")
if p.returncode:
    errors.append("git diff failed")
else:
    additions="\n".join(line[1:] for line in p.stdout.splitlines() if line.startswith("+") and not line.startswith("+++"))
    for pat in FORBIDDEN_ADDITIONS:
        if re.search(pat, additions, flags=re.I):
            errors.append(f"forbidden scope signal in added lines: {pat}")

# Core contract invariants should have executable regression coverage.
core=(ROOT/"product/shark-books-core/src/lib.rs").read_text(encoding="utf-8",errors="replace").lower()
for phrase in [
    "sbc8a3_credit_reduces_balance_without_fabricating_payment",
    "sbc8a3_credit_preserves_source_line_and_remaining_quantity",
    "sbc8a3_balance_handles_integer_boundary",
]:
    if phrase not in core:
        errors.append(f"missing regression test: {phrase}")

out={"result":"PASS" if not errors else "FAIL","errors":errors,"allowed_paths":len(ALLOWED)}
print(json.dumps(out,indent=2))
sys.exit(0 if not errors else 1)
