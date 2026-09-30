from __future__ import annotations
import json,subprocess,sys
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
CONTRACT=Path(r"C:\SharkAutopilot\lanes\SBC\evidence\SBC8A1_CONTRACT_FREEZE.json")
def git(*a): return subprocess.run(["git",*a],cwd=str(ROOT),text=True,capture_output=True,check=True).stdout.strip()
def main():
 e=json.loads(CONTRACT.read_text(encoding="utf-8-sig")); problems=[]
 base=e["entry_sha"]; allowed=set(e["allowed_paths"])
 changed=set(filter(None,git("diff","--name-only",base).splitlines()))
 bad=sorted(changed-allowed)
 if bad:problems.append({"CHANGED_PATH_OUTSIDE_CONTRACT":bad})
 locks=[p for p in changed if Path(p).name in ("Cargo.lock","package-lock.json","pnpm-lock.yaml","yarn.lock")]
 if locks:problems.append({"DEPENDENCY_LOCK_CHANGED":locks})
 markers={
 "product/shark-books-core/src/lib.rs":["CommercialPartySnapshot"],
 "workspace/shark-foundation/src/contact_application.rs":["postal_address","email","phone"],
 "workspace/shark-foundation/src/bank_application/mod.rs":["APPLICATION_SCHEMA_VERSION"],
 "workspace/shark-tauri-spike/src/owner_supporting_data.rs":["postal_address","email","phone"],
 "workspace/ui/src/screens/ContactsScreen.vue":["postalAddress","email","phone"]}
 for rel,need in markers.items():
  p=ROOT/rel
  if not p.is_file():problems.append({"MISSING_FILE":rel});continue
  txt=p.read_text(encoding="utf-8",errors="replace")
  miss=[x for x in need if x not in txt]
  if miss:problems.append({"MISSING_MARKERS":{rel:miss}})
 print(json.dumps({"result":"PASS" if not problems else "FAIL","base_sha":base,"changed_paths":sorted(changed),"problems":problems},indent=2))
 return 0 if not problems else 2
if __name__=="__main__":raise SystemExit(main())
