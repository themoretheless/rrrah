"""Verify the completed human source/image review and independently requery corpus candidates."""
from pathlib import Path
import ast,hashlib,json,sys
R=Path(__file__).resolve().parents[1];D=R/'docs/research/dedup-500';W=R/'.dedup-work/near41-20261015';S=W/'source'
h=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
load=lambda n:json.loads((D/n).read_text())
rows=load('near41-full-file-pins-20261015.json');pin=load('near41-upstream-tree-20261015.json');old=load('near41-partial-source-native-review-20261015.json');corpus=load('near41-visual-corpus-prepared-20261015.json')
assert len(rows)==594 and not pin['truncated']
for row in rows:
 b=(S/row['path']).read_bytes();assert len(b)==row['size'] and hashlib.sha256(b).hexdigest()==row['sha256'];assert hashlib.sha1(b'blob '+str(len(b)).encode()+b'\0'+b).hexdigest()==row['sha']
 if 'content' in row:assert b==row['content'].encode()
trees={'':pin['root_tree'],**{x['path']:x['sha'] for x in pin['tree'] if x['type']=='tree'}}
for parent,sha in trees.items():
 es=[dict(x,path=Path(x['path']).name) for x in pin['tree'] if ('' if str(Path(x['path']).parent)=='.' else str(Path(x['path']).parent))==parent];es.sort(key=lambda x:x['path']+('/' if x['type']=='tree' else ''))
 b=b''.join(x['mode'].lstrip('0').encode()+b' '+x['path'].encode()+b'\0'+bytes.fromhex(x['sha']) for x in es);assert hashlib.sha1(b'tree '+str(len(b)).encode()+b'\0'+b).hexdigest()==sha
text=[x['path'] for x in rows if 'content' in x];assert len(text)==49 and len(trees)==17
n=next(n for n in ast.parse((S/'versioneer.py').read_text()).body if isinstance(n,ast.Assign) and isinstance(n.value,ast.Constant) and isinstance(n.value.value,str) and len(n.value.value)>10000)
rendered=n.value.value % {'DOLLAR':'$','STYLE':'pep440','TAG_PREFIX':'','PARENTDIR_PREFIX':'deduplicate-','VERSIONFILE_SOURCE':'src/deduplication/_version.py'}
assert rendered==(S/'src/deduplication/_version.py').read_text()
receipt=load('near41-visual-view-receipt-20261015.json');assert receipt['human_viewed_sheet_ids']==list(range(13))
ids=[]
for i,s in enumerate(corpus['sheets']):
 assert h(Path(s['path']))==s['sha256']==receipt['sheet_sha256'][i];ids+=s['visual_ids']
assert ids==list(range(502))
records=corpus['records'];assert len(records)==540 and len({r['git_blob'] for r in records})==502
by_path={x['path']:x for x in rows}
for r in records:
 p=by_path[r['path']];assert r['git_blob']==p['sha'] and r['sha256']==p['sha256'];assert int(r['phash'],16)==r['hash_integer'] and r['unique_visual_id'] in ids
for path,sha in old['logs'].items():assert h(R/path)==sha
for i in (1,2):
 result=json.loads((D/f'near41-native-controls-run{i}-20261015.log').read_text().splitlines()[-1]);assert result['status']=='passed' and result['controls']==12
assert h(R/old['dependency_pins'])==old['dependency_pins_sha256']
deps=load('near41-installed-dependency-pins-20261015.json')
for p in deps['installed_payloads']:assert h(Path(p['path']))==p['sha256']
sys.path.insert(0,str(W/'deps'))
import numpy as np
from sklearn.neighbors import KDTree
from PIL import Image
import imagehash
for r in records:
 with Image.open(S/r['path']) as im:
  assert list(im.size)==r['size'];assert hashlib.sha256(im.convert('RGB').tobytes()).hexdigest()==r['rgb_pixel_sha256'];assert str(imagehash.phash(im))==r['phash']
x=np.array([[int(d,16) for d in r['phash']] for r in records]);dist,ix=KDTree(x,metric='manhattan').query(x,k=5)
edges=hits=omissions=0
for i,r in enumerate(records):
 expected={j for j,q in enumerate(records) if i!=j and (r['hash_integer']^q['hash_integer']).bit_count()<=8}
 retrieved={int(j) for j,d in zip(ix[i],dist[i]) if j!=i and d<=40};edges+=len(expected);hits+=len(expected&retrieved);omissions+=bool(expected-retrieved)
assert (edges,hits,omissions)==(2902,1429,334)
for k,v in [('directed_hamming_radius_edges',edges),('retrieved_directed_hamming_radius_edges',hits),('queries_with_omissions',omissions)]:assert corpus['native_corpus_diagnostic'][k]==v
out=D/'near41-complete-source-review-20261015.json';assert not out.exists()
result={**old,'status':'complete_pinned_source_and_visual_asset_review_scoped_native_controls','whole_repo_complete':True,'full_repository_read':True,'fully_read_text':sorted(text),'text_read_remaining':[],'unviewed_dataset_images':0,'visual_coverage':{'dataset_paths':540,'unique_dataset_blobs_viewed':502,'byte_identical_alias_paths':38,'tracked_docs_images_viewed':5,'contact_sheets_viewed':13,'receipt':'docs/research/dedup-500/near41-visual-view-receipt-20261015.json'},'versioneer_template':{'lines':[n.lineno,n.end_lineno],'exactly_equals_fully_read_version_file':True,'method':'AST literal interpolation only; no versioneer execution'},'prior_partial_review':'docs/research/dedup-500/near41-partial-source-native-review-20261015.json','corpus_diagnostic':corpus['native_corpus_diagnostic'],'independent_verification':{'all_git_blobs':594,'all_git_trees':17,'installed_dependency_payloads':len(deps['installed_payloads']),'pixel_and_phash_recomputed':540,'native_kdtree_requery':True},'scope':'All49textblobs read, all545image paths visually covered via507unique originalimages and38 byte-identical aliases. Native12controls twice and540-image diagnostic; original2019 environment/upstreamsuite/fullCLI/multiprocessing/platform/visualgroundtruth/performance remain unqualified. No production adoption or speedup.','remaining':['Original dependency/upstream suite/full CLI/multiprocessing/platform qualification','Calibrated positive/negative visual pair truth; Hamming radius is only a hash-space oracle','Root production adoption and measured whole-library speedup remain separate gates']}
result['findings']+=['On all540upstreamdatasetimages, k5 numericManhattan<=40 retrieves1429of2902directed Hamming<=8edges;334querieshaveomissions. Explicit differing metric/radius policies, not a measured visual-quality score.','Datasetout.csv is generationcount metadata, not pairgroundtruth. Package __init__ prints and rewrites sys.path; modern Versioneer compatibility unexecuted.']
result['evidence_pins']={str(p.relative_to(R)):h(p) for p in [D/'near41-full-file-pins-20261015.json',D/'near41-upstream-tree-20261015.json',D/'near41-visual-corpus-prepared-20261015.json',D/'near41-visual-view-receipt-20261015.json',Path(__file__)]}
out.write_text(json.dumps(result,indent=2)+'\n')
p=D/'reviews/041.json';prior=D/'near41-prior-completion-registry-20261015.json';assert not prior.exists();prior.write_bytes(p.read_bytes());reg=json.loads(p.read_text());reg.update(full_repository_read=True,complete_source_review=str(out.relative_to(R)),review_scope='Complete pinned49text/545image source and asset review;12scopednativecontrols twice and540image candidate diagnostic. Original upstream suite and calibrated visual-quality/performance unqualified.',license_scope='Full tracked Apache2 LICENSE read; Versioneer CC0/publicdomain declarations read. No source copied into production.',finding='Reference only: metric mismatch, self-inclusive topk radius omissions, greedy retention and backup-before-delete safety findings; no index substitution without recall/quality gates.');p.write_text(json.dumps(reg,indent=2)+'\n')
print(json.dumps({'status':'verified','blobs':594,'trees':17,'text':49,'images':545,'corpus_edges':edges,'retrieved':hits,'queries_with_omissions':omissions}))
