"""Verify all fixed16 completed controls without converting unknowns into negatives."""
import pathlib,json,hashlib
R=pathlib.Path(__file__).resolve().parents[1];D=R/'docs/research';P=D/'dedup-current-heldout-negatives-20261012.json';A=D/'dedup-current-heldout-negatives-prefix-16-audit-20261012.json';d=json.loads(P.read_text());a=json.loads(A.read_text())
def sha(p):return hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()
assert d['status']=='complete_current_fixed16_execution' and d['completed']==16 and len(d['results'])==len(d['plan'])==16
assert a['completed']==a['required']==16 and a['producer_status_at_capture']==d['status'];assert all(sha(p)==h for p,h in a['pins'].items())
capture=next(pathlib.Path(p) for p in a['pins'] if 'heldout-negatives-prefix-16-' in p);assert capture.read_bytes()==P.read_bytes();assert a['counts']==d['counts']=={'false_acceptance':0,'inconclusive':16,'rejected':0,'execution_failed':0}
out={'status':'verified_complete_current_fixed16_execution','completed':16,'counts':a['counts'],'pins':{str(p):sha(p) for p in [P,A,pathlib.Path(__file__)]},'scope':'All16 original fixed published-label hash-near distractors on the earlier frozen optimized source. No false acceptance observed;16unknowns and0rejections do not establish negative precision/recall or semantic-heldout generality.'};(D/'dedup-current-heldout-negatives-complete-audit-20261013.json').write_text(json.dumps(out,indent=2)+'\n');print(out['status'])
