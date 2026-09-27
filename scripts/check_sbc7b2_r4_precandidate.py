#!/usr/bin/env python3
from __future__ import annotations
import argparse, json, re, subprocess, sys
from pathlib import Path

TARGETS = {
    "MONEY.RECORDS.LIST": "owner_money_records_list -> Books::owner_money_records",
    "MONEY.RECORD.DETAIL": "owner_money_record_detail -> Books::owner_money_record_detail",
    "BANK.ACTIVITY_DETAIL": "owner_bank_activity_detail -> owner-safe bank activity view",
    "DOCUMENT.LIST": "owner_document_list -> Books::owner_documents",
    "DOCUMENT.OPEN_VIEW": "owner_document_open_view -> bounded registered-document integrity read",
}
EXPOSED = {
    "BOOKS.CREATE","BOOKS.OPEN","BOOKS.VERIFY","HOME.STATUS","MONEY_IN.PREVIEW","MONEY_IN.SAVE",
    "MONEY_OUT.PREVIEW","MONEY_OUT.SAVE","MONEY.RECORDS.LIST","MONEY.RECORD.DETAIL","CORRECTION.PREVIEW",
    "CORRECTION.CONFIRM","CORRECTION.HISTORY","BANK.IMPORT_PREVIEW_CSV","BANK.IMPORT_PREVIEW_OFX_QFX",
    "BANK.IMPORT_CONFIRM_CSV","BANK.IMPORT_CONFIRM_OFX_QFX","BANK.ACTIVITY_LIST","BANK.ACTIVITY_DETAIL",
    "BANK.MATCH_REVIEW","BANK.MATCH_CONFIRM","BANK.RECONCILE_PREVIEW","BANK.RECONCILE_FINALISE",
    "DOCUMENT.SELECT_REGISTER","DOCUMENT.VERIFY","DOCUMENT.LIST","DOCUMENT.OPEN_VIEW","DOCUMENT.ATTACH",
    "OCR.EXTRACT_RECEIPT","RECEIPT.SUGGEST_BANK","RECEIPT.CONFIRM_BANK","RECEIPT.REJECT_BANK",
    "CONTACTS.LIST","CONTACTS.SAVE","REPORT.SUMMARY","SETTINGS.BOOKS_INFO","SETTINGS.STORAGE_ROOT.SELECT",
}

def require(ok: bool, msg: str):
    if not ok: raise AssertionError(msg)
    print(f"PASS: {msg}")

def read(repo: Path, rel: str) -> str:
    p=repo/rel; require(p.is_file(), f"required path exists: {rel}"); return p.read_text(encoding='utf-8')

def registry(repo: Path):
    root=repo/'workspace/shark-foundation/data'; rows=[]
    manifest=json.loads((root/'action_registry_v1_manifest.json').read_text(encoding='utf-8'))
    for i in range(1,25):
        p=root/f'action_registry_v1_chunk{i:02d}.jsonl'; require(p.is_file(), f"registry chunk {i:02d} exists")
        for line in p.read_text(encoding='utf-8').splitlines():
            if line.strip(): rows.append(json.loads(line))
    return manifest, rows

def main():
    ap=argparse.ArgumentParser(); ap.add_argument('--repo', default='.'); ap.add_argument('--skip-git', action='store_true'); a=ap.parse_args()
    repo=Path(a.repo).resolve()
    try:
        manifest, rows=registry(repo); by={r['action_id']:r for r in rows}
        require(manifest['schema']=='sharkbooks-action-registry-v1','registry schema remains v1')
        require(len(rows)==206 and len(by)==206,'registry remains 206 unique canonical actions')
        voice=[r for r in rows if r['voice_eligible']]
        require(len(voice)==175,'voice-eligible count remains 175')
        require(sum(len(r['utterances']) for r in voice)==700,'finite utterance count remains 700')
        for aid,basis in TARGETS.items():
            r=by[aid]
            require(r['implementation_state']=='SBC7B2_DEVELOPMENT_BRANCH', f'{aid} lifecycle reconciled to development branch')
            require(r['availability_state']=='OWNER_UI_BOUND_PRE_CANDIDATE', f'{aid} exposure reconciled to owner UI pre-candidate')
            require(r['evidence_state']=='WINDOWS_DEVELOPMENT_PASS', f'{aid} evidence truth remains Windows development-pass only')
            require(r['voice_parity']=='REQUIRED_WHEN_EXPOSED', f'{aid} text/speech parity policy is exposure-aware')
            require(r['backend_state']=='READY', f'{aid} backend state is READY')
            require(r['backend_basis']==basis, f'{aid} backend basis names the proven bridge')
        native=read(repo,'workspace/shark-tauri-spike/src/owner_command_text.rs')
        block=re.search(r'COMMAND_TEXT_EXPOSED_ACTION_IDS:\s*&\[&str\]\s*=\s*&\[(.*?)\];',native,re.S)
        require(block is not None,'command-text exposure constant exists')
        ids=set(re.findall(r'"([A-Z0-9_.]+)"',block.group(1)))
        require(ids==EXPOSED,'command-text exposure remains exactly the 37 UI-A/UI-B actions')
        require(all(by[x]['backend_state']=='READY' for x in EXPOSED),'all 37 exposed command actions are controller-ready after R4')
        require('stale LOCKED registry metadata' not in native,'R3 stale-lock comment removed')
        require('reconciled_document_open_is_command_text_executable_with_owner_fact' in native,'native command-text test covers reconciled document open')
        part05=read(repo,'workspace/shark-foundation/src/action_system/part05.rs')
        require('reconciled_read_view_action_is_executable' in part05,'Foundation command-text test covers reconciled read/view readiness')
        require('exposed_but_stale_locked_read_view_action_stays_fail_closed' not in part05,'stale R3 lock test is superseded')
        r3=read(repo,'scripts/check_sbc7b2_r3_ft4.py')
        require('R4_RECONCILED_READY' in r3 and 'backend_state"] == "READY"' in r3,'R3 static checker is reconciled to R4 state')
        for rel in ['workspace/ui/src/generated/uia-actions.ts','workspace/ui/src/generated/uib-actions.ts']:
            text=read(repo,rel)
            for aid in TARGETS:
                if aid in text:
                    m=re.search(r'"actionId":\s*"'+re.escape(aid)+r'".*?"backendState":\s*"([A-Z_]+)"',text,re.S)
                    require(m is not None and m.group(1)=='READY', f'{rel} reflects READY for {aid}')
        win=read(repo,'ci/run_sbc7b2_supergate_windows.ps1'); mac=read(repo,'ci/run_sbc7b2_supergate_codemagic.sh'); cm=read(repo,'codemagic.yaml')
        for text,label in [(win,'Windows'),(mac,'Apple')]:
            require('99c3b0d16fe4934f1b397a58956df745203f744f' not in text, f'{label} runner no longer uses pre-FT1 base')
            require('ebf1ae9d0e10f0f427625d4c3ab5c1703299d154' in text, f'{label} runner uses protected FT1/FT2 merged base')
            require('NOT_APPLICABLE_TO_THIS_CANDIDATE' not in text, f'{label} runner no longer waives SG3/SG4')
            for gate in ['SG3_UI_A','SG4_UI_B','SG5_COMMAND_TEXT']:
                require(gate in text, f'{label} runner contains mandatory {gate}')
            require('check_sbc7b2_r4_precandidate.py' in text, f'{label} runner invokes integrated R4 checker')
            require('owner_command_text::tests' in text and 'command_text_tests' in text, f'{label} SG5 runs actual finite command-text tests')
        require('sbc7b2-ft3-ft4-supergate-apple:' in cm,'Codemagic exposes integrated FT3/FT4 Apple workflow')
        require('SBC7B2_FT3_FT4_SUPERGATE_MAC' in cm,'Codemagic collects integrated FT3/FT4 evidence')
        flow=json.loads(read(repo,'docs/SBC7B2_R4_DATA_FLOW_2026-09-21.json'))
        require(flow['network_expansion'] is False,'R4 introduces no network expansion')
        require(flow['new_dependencies']==[],'R4 introduces no dependencies')
        require(flow['new_permissions']==[],'R4 introduces no permissions')
        require(flow['public_release_change'] is False,'R4 introduces no public-release change')
        require(flow['new_off_device_data_recipients']==[],'R4 introduces no off-device data recipient')
        for rel in ['docs/SBC7B2_R4_PRIVACY_DELTA_2026-09-21.md','docs/SBC7B2_R4_PRIVACY_CUMULATIVE_2026-09-21.md','docs/SBC7B2_R4_USER_WORDING_IMPACT_2026-09-21.md']:
            require((repo/rel).is_file(), f'privacy evidence exists: {rel}')
        if not a.skip_git:
            head=subprocess.check_output(['git','-C',str(repo),'rev-parse','HEAD'],text=True,encoding='utf-8').strip()
            base='ebf1ae9d0e10f0f427625d4c3ab5c1703299d154'
            subprocess.run(['git','-C',str(repo),'merge-base','--is-ancestor',base,head],check=True)
            require(not subprocess.check_output(['git','-C',str(repo),'status','--porcelain'],text=True,encoding='utf-8').strip(),'repository is clean')
    except Exception as exc:
        print(f'[FAIL] {exc}',file=sys.stderr); return 1
    print('[PASS] SBC-7B2 R4 integrated pre-candidate reconciliation V6 Rust parity-validator reconciliation contract')
    return 0
if __name__=='__main__': raise SystemExit(main())
