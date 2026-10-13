"""Bind closed portfolio native controls and test-only dependency coverage to hashes."""
import hashlib,json,shutil
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];D=ROOT/'docs/research'
OUT=D/'dedup-projected-portfolio-native-audit-20261011.json'
ARCHIVE=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays/projected-portfolio-final-audit-20261011')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 assert not OUT.exists() and not ARCHIVE.exists()
 files=[D/'dedup-projected-portfolio-native-20261011.json',D/'dedup-projected-portfolio-generic-20261011.json',D/'dedup-projected-portfolio-dependency-generic-20261011.json']
 native,generic,dependency=[json.loads(p.read_text()) for p in files]
 assert native['status']=='complete_bounded_eight_native_projected_portfolio_controls_execution'
 assert native['all_expected_outcomes_match'] and native['all_independent_projection_evidence_exact']
 assert len(native['results'])==8 and all(r['status']=='passed' and r['returncode']==0 and r['evidence']['managed_used']==0 for r in native['results'])
 assert generic['status']=='complete_bounded_frozen_projected_portfolio_generic_execution' and generic['tests']==131
 assert dependency['status']=='complete_bounded_frozen_projected_portfolio_dependency_execution' and dependency['tests']==13
 for report in [native,generic,dependency]:
  assert all(sha(p)==h for p,h in report['pins'].items())
 for report in [generic,dependency]:
  for result in report['results']:
   assert result['status']=='passed' and result['returncode']==0
   assert sha(result['binary'])==result['binary_sha256'] and sha(result['log'])==result['log_sha256']
 manifests=[D/'dedup-projected-portfolio-snapshot-20261011.json',D/'dedup-projected-portfolio-dependency-snapshot-20261011.json']
 parent,child=[json.loads(p.read_text()) for p in manifests]
 for manifest in [parent,child]:
  assert all(sha(Path(manifest['snapshot'])/p)==h for p,h in manifest['file_hashes'].items())
 delta=[p for p,h in child['file_hashes'].items() if parent['file_hashes'].get(p)!=h]
 assert delta==['crates/rrrah-dedup/tests/projected_portfolio.rs']
 assert all(sha(ROOT/p)==h for p,h in child['file_hashes'].items() if p.startswith('crates/rrrah-dedup/src/'))
 ARCHIVE.mkdir();pins={}
 for path in [*files,*manifests,Path(__file__)]:
  pins[str(path)]=sha(path);target=ARCHIVE/path.name;shutil.copy2(path,target);assert sha(target)==pins[str(path)]
 audit={'status':'complete_bounded_current_projected_portfolio_native_and_dependency_qualification','native_invocations':8,'native_pair_records':10,'native_all_expected_outcomes_match':True,'native_independent_projection_evidence_exact':True,'parent_generic_tests':131,'test_only_dependency_suite_tests':13,'distinct_current_generic_tests':132,'dependency_only_delta':delta,'frozen_files':len(child['file_hashes']),'current_dedup_production_hashes_match':True,'pins':pins,'archive':str(ARCHIVE),'limits':['No broad origin-negative/burst precision, indexed 386-file collection or all-format qualification','No hard total Torch tensor/workspace/RSS admission','Linux current gate still running; no Linux/Windows Torch execution','Full229 is a separate earlier frozen source, not evidence for all current portfolio paths']}
 OUT.write_text(json.dumps(audit,indent=2)+'\n');print(audit['status'])
if __name__=='__main__':main()
