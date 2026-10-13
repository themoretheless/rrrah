#!/usr/bin/env python3
import hashlib,json
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research/dedup-500'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 mp=D/'fclones-hasher-source-20261012.json';m=json.loads(mp.read_text());S=Path(m['snapshot']);assert len(m['file_hashes'])==42 and all(sha(S/p)==h for p,h in m['file_hashes'].items())
 names=['fclones-upstream-verification-20261012.json','fclones-cache-controls-audit-20261012.json','fclones-group-cache-audit-20261012.json','fclones-walk-review-v2-20261012.json','fclones-path-selector-transform-review-20261012.json','fclones-progress-utils-review-20261012.json','fclones-pattern-regex-review-v2-20261012.json','fclones-arguments-review-20261012.json','fclones-config-review-20261012.json','fclones-error-log-main-review-20261012.json','fclones-packaging-ci-review-20261012.json','fclones-readme-review-20261012.json','fclones-report-review-20261012.json','fclones-dedupe-review-20261012.json','fclones-reflink-generator-lock-review-20261012.json'];coverage={};pins={str(mp):sha(mp),str(Path(__file__)):sha(Path(__file__))}
 for name in names:
  p=D/name;v=json.loads(p.read_text());pins[str(p)]=sha(p)
  for k,h in v.get('pins',{}).items():assert sha(k)==h,(name,k)
  for field in ['fully_read_source_files','fully_read_source_files_additional','fully_read_additional','fully_read_additional_files']:
   paths=v.get(field,[])
   for path in paths:
    assert path in m['file_hashes'];coverage.setdefault(path,[]).append({'review':name,'method':'complete_source_text_read'})
    if isinstance(paths,dict):assert len((S/path).read_text().splitlines())==paths[path],(name,path)
  for path,details in v.get('machine_inspected_files',{}).items():
   assert path in m['file_hashes'];coverage.setdefault(path,[]).append({'review':name,'method':'complete_generated_lock_structured_review','details':details})
 assert set(coverage)==set(m['file_hashes']),set(m['file_hashes'])-set(coverage)
 assert len([p for p in coverage if p!='Cargo.lock'])==41
 out=D/'fclones-complete-source-review-20261012.json';assert not out.exists();out.write_text(json.dumps({'status':'complete_pinned42file_tree_review_with_explicit_runtime_limits','repository':m['repository'],'commit':m['commit'],'files_reviewed':42,'complete_text_reads':41,'complete_structured_generated_lock_reviews':1,'coverage':coverage,'pins':pins,'adopted_implementation':False,'conclusions':['Staged size/prefix/suffix/full hashing, per-device I/O scheduling and compressed shared-parent paths are useful design references, not rrrah exact-proof admission.','Do not transfer metadata-only cache trust, hash-only final grouping, lossy/trimmed path handling, unchecked report numeric parsing or cleanup lock/transaction assumptions.','Existing rrrah exact contract retains full content hash plus byte confirmation, explicit incomplete/error/cancel results and fresh mutation checks. New whitespace-name regression already qualified on macOS; implementation unchanged.'],'runtime_evidence_scope':'Original pinned upstream170unit and2doc passes are historical macOS ARM64 evidence; one Linux-only physical-device unit ignored. Later targeted original suites and independent helper/public-API controls are separately pinned and overlap original counts; do not add them into a new total. Temporary test fixtures only; dry-plan counterexample commands never executed. Some independent harnesses use compatible offline dependencies and are labeled accordingly.','limitations':['No independent source audit of197registry dependencies; generated Cargo.lock metadata/graph inspected only.','No full physical Linux/Windows reflink, concurrency/advisory-lock race, ACL/xattr failure preservation or hard total-memory qualification.','No CI/package/release execution or contemporary benchmark confirmation.','This completes pinned fclones tree review only; it does not complete all500repositories, full458positive corpus, broad semantic negatives, formats/platforms or whole duplicate-library goal.']},indent=2)+'\n');print('Verified complete42file pinned fclones review:41text+1structured lock; no implementation adopted')
if __name__=='__main__':main()
