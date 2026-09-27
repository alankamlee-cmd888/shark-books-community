from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]

def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")

def require(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(f"FAIL: {message}")
    print(f"PASS: {message}")

foundation = read("workspace/shark-foundation/src/owner_read_application.rs")
foundation_lib = read("workspace/shark-foundation/src/lib.rs")
tauri_read = read("workspace/shark-tauri-spike/src/owner_read_views.rs")
doc_bridge = read("workspace/shark-tauri-spike/src/owner_documents_ocr.rs")
tauri_lib = read("workspace/shark-tauri-spike/src/lib.rs")
tauri_read_production = tauri_read.split("#[cfg(test)]", 1)[0]
build = read("workspace/shark-tauri-spike/build.rs")
permission = read("workspace/shark-tauri-spike/permissions/shark-shell.toml")

commands = [
    "owner_money_records_list",
    "owner_money_record_detail",
    "owner_bank_activity_detail",
    "owner_document_list",
    "owner_document_open_view",
]

require("mod owner_read_application;" in foundation_lib, "Foundation owner read module is declared")
require("pub use owner_read_application::" in foundation_lib, "Foundation owner read types are explicitly re-exported")
require("sbc7b1:moneyIn:" in foundation and "sbc7b1:moneyOut:" in foundation, "Money list is bounded to canonical owner record references")
require("shark_owner_correction" in foundation, "Money list excludes records superseded by correction history")
require("correctionReversal" in foundation, "Money list documents the distinct reversal reference boundary")
require("OwnerMoneyRecordRead" in foundation and "OwnerMoneyRecordDetailRead" in foundation, "Owner-safe Money read models exist")
require("EntryView" not in tauri_read and "PostingLine" not in tauri_read, "Tauri read bridge receives no ledger-shaped entry/posting DTO")

for forbidden in [
    "storage_root_id",
    "relative_path",
    "sha256",
    "source_file_sha256",
    "source_locator",
    "raw_record_sha256",
    "matched_entry_id",
    "institution_account_id",
    "provenance_fingerprint",
]:
    require(forbidden not in tauri_read_production, f"Owner read bridge omits raw/internal field {forbidden}")

require("require_verified_document" in doc_bridge, "Document open reuses trusted-root integrity verification")
require("read_bounded_file" in doc_bridge, "Document open retains bounded native file read")
require("sha256_hex" in doc_bridge, "Document open rechecks content hash after the final read")
require("tauri::ipc::Response::new(bytes)" in doc_bridge, "Document open returns native binary IPC bytes rather than a path")

for command in commands:
    require(command in tauri_lib, f"Runtime handler registers {command}")
    require(f'"{command}"' in build, f"Tauri build manifest registers {command}")
    require(f'"{command}"' in permission, f"Shell permission allows {command}")

request_blocks = re.findall(r"#\[derive\([^\]]*Deserialize[^\]]*\)\].*?struct\s+\w+Request\s*\{(.*?)\n\}", tauri_read_production, flags=re.S)
request_text = "\n".join(request_blocks)
for forbidden in [
    "database_path",
    "passphrase",
    "account_code",
    "direction",
    "tax_category",
    "source_path",
    "file_url",
    "storage_root_path",
]:
    require(forbidden not in request_text, f"Read requests omit authority field {forbidden}")

print("PASS: SBC-7B2 FT3 owner-safe read/view static contract")
