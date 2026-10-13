#!/usr/bin/env python3
import hashlib,json,subprocess
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research/dedup-500'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 rp=D/'anew-full-review-20261012.json';r=json.loads(rp.read_text());assert all(sha(k)==v for k,v in r['pins'].items());mp=D/'anew-full-source-20261012.json';m=json.loads(mp.read_text());s=Path(m['snapshot']);paths=subprocess.check_output(['git','-C',str(s),'ls-tree','-r','--name-only',m['commit']],text=True).splitlines();assert set(paths)==set(m['file_hashes'])==set(m['fully_read'])and len(paths)==6
 for k in paths:
  assert sha(s/k)==m['file_hashes'][k];assert subprocess.check_output(['git','-C',str(s),'cat-file','blob',m['commit']+':'+k])==(s/k).read_bytes()
 assert len(r['controls'])==6 and all(x['returncode']==0 and x['stderr']==''for x in r['controls']);by={x['control']:x for x in r['controls']};assert by['long_stdin_token_silently_discards_remaining_input']['stdout']=='first\n';row=by['long_existing_token_misses_later_membership_and_appends_duplicate'];assert row['stdout']=='later\n'and row['after_length']==row['before_length']+6;assert by['dry_run_preserves_target']['before_sha256']==by['dry_run_preserves_target']['after_sha256'];assert r['upstream_go_test']=='success_no_test_files_not_unit_passes'and r['decision']=='reject-domain_no_production_adoption'
 out=D/'anew-full-review-audit-20261012.json';assert not out.exists();out.write_text(json.dumps({'status':'audited_complete_six_file_review_six_original_cli_controls','registry_index':7,'commit':m['commit'],'pins':{str(p):sha(p)for p in [rp,mp,Path(__file__)]},'scope':r['scope']},indent=2)+'\n');print('Audited pinned complete6file source and6originalCLIcontrols; no upstream unit tests claimed')
if __name__=='__main__':main()
