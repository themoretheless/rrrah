"""Verify complete file pins but explicitly refuse full-source completion while review remains."""
from pathlib import Path
import base64,hashlib,json,re,sys
R=Path(__file__).resolve().parents[1];D=R/'docs/research/dedup-500';W=R/'.dedup-work/near41-20261015';S=W/'source';h=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
pin=json.loads((D/'near41-upstream-tree-20261015.json').read_text());rows=json.loads((D/'near41-full-file-pins-20261015.json').read_text());assert len(rows)==594 and not pin['truncated']
for row in rows:
 p=S/row['path'];b=p.read_bytes();assert len(b)==row['size'];assert hashlib.sha256(b).hexdigest()==row['sha256'];assert hashlib.sha1(b'blob '+str(len(b)).encode()+b'\0'+b).hexdigest()==row['sha']
 if 'content' in row:assert b==row['content'].encode()
trees={'':pin['root_tree']};trees.update({x['path']:x['sha'] for x in pin['tree'] if x['type']=='tree'})
for parent,sha in trees.items():
 es=[dict(x,path=Path(x['path']).name) for x in pin['tree'] if ('' if str(Path(x['path']).parent)=='.' else str(Path(x['path']).parent))==parent];es.sort(key=lambda x:x['path']+('/' if x['type']=='tree' else ''))
 b=b''.join(x['mode'].lstrip('0').encode()+b' '+x['path'].encode()+b'\0'+bytes.fromhex(x['sha']) for x in es);assert hashlib.sha1(b'tree '+str(len(b)).encode()+b'\0'+b).hexdigest()==sha
runs=[];logs={}
for i in [1,2]:
 p=D/f'near41-native-controls-run{i}-20261015.log';lines=p.read_text().splitlines();r=json.loads(lines[-1]);assert r['status']=='passed' and r['controls']==12;assert [s[5:] for s in lines if s.startswith('PASS ')]==r['cases'];assert r['observations']['nibble_numeric_vs_bit_hamming']['pairs']==256;assert r['observations']['nibble_numeric_vs_bit_hamming']['false_positive_directed_edges']==14 and r['observations']['nibble_numeric_vs_bit_hamming']['missing_directed_edges']==48
 for f,sha in r['source_pins'].items():assert h(S/'src/deduplication'/f)==sha
 runs.append(r);logs[str(p.relative_to(R))]=h(p)
 q=D/f'near41-native-controls-run{i}-20261015.stderr.log';logs[str(q.relative_to(R))]=h(q)
assert runs[0]['cases']==runs[1]['cases'] and runs[0]['observations']==runs[1]['observations']
sys.path.insert(0,str(W/'deps'))
import importlib.metadata as metadata
payloads=[];distributions=[];missing=[]
for name in ['numpy','pandas','scipy','scikit-learn','Pillow','ImageHash','tqdm','natsort','joblib','threadpoolctl']:
 dist=metadata.distribution(name);distributions.append({'name':dist.metadata['Name'],'version':dist.version})
 for entry in dist.files or []:
  p=Path(dist.locate_file(entry))
  if p.suffix=='.pyc':continue
  if p.is_file():payloads.append({'path':str(p.resolve()),'bytes':p.stat().st_size,'sha256':h(p)})
  else:missing.append(str(p))
deps=D/'near41-installed-dependency-pins-20261015.json';assert not deps.exists();deps.write_text(json.dumps({'distributions':distributions,'installed_payloads':payloads,'missing_declared_entries':missing,'scope':'Existing nonpyc installeddistributionpayloads hashed; not full dependency source audit, original2019env or all linked framework qualification.'},indent=2)+'\n')
unread=['LICENSE','versioneer.py','src/deduplication/_version.py','src/deduplication/__init__.py','datasets/caltech_101_rand_resize/out.csv'];read=sorted(x['path'] for x in rows if 'content' in x and x['path'] not in unread);assert len(read)==44
visuals=[x['path'] for x in rows if x['path'].startswith('docs/images/')];assert len(visuals)==5
out=D/'near41-partial-source-native-review-20261015.json';assert not out.exists();out.write_text(json.dumps({'status':'partial_source_read_native_metric_radius_graph_controls','repository':'umbertogriffo/fast-near-duplicate-image-search','index':41,'commit':pin['commit'],'root_tree':pin['root_tree'],'whole_repo_complete':False,'full_repository_read':False,'tracked_blobs':594,'tracked_trees':len(trees),'all_object_hashes_verified':True,'text_blobs':49,'fully_read_text':read,'text_read_remaining':unread,'binary_images':545,'viewed_images':visuals,'unviewed_dataset_images':540,'source_archive_substitution':'GitHubexport-subst changed _version.py18466->18514bytes; extractedcopyrejectedbeforecompletemanifest. ExactoriginalGitblob fetched/verified andstored separately; all594blobs nowmatchtree.','native_controls':12,'runs':2,'logs':logs,'observations':runs[0]['observations'],'dependency_pins':str(deps.relative_to(R)),'dependency_pins_sha256':h(deps),'script':'scripts/qualify-near41-native-controls-20261015.py','script_sha256':h(R/'scripts/qualify-near41-native-controls-20261015.py'),'adapters':runs[0]['adapters'],'scope':'Selected unchangedwholeclasses/functions with actualsklearn/scipy/Pandas/Pillow/ImageHash/nativeownedFS. Syntheticnumeric hashvectors, notvisualgroundtruth. All5READMEtrackeddiagrams/images viewed: pHash/index/search/deleteflow, fivepotatoesresult andtSNEcluster. Dataset540images notviewed, remaining5textblobs notread: cannotclaimcomplete repository or incrementfullcounter.','findings':['Topk includes selfandcanmissotherradiusmatches; equalhashfirstneighboranchor loses2of4rows withk2.','Greedy chain removesrowwithoutretainedneighborwithindistance threshold.','NumerichexManhattan differsfrombitHamming:256directednibblepairs/14extra48missingedgesatthreshold1; nativeKDTreecorrectforitsownnumericmetric.','OriginalcKDTree n_jobs incompatiblewithmodernSciPy; explicitkeyword-onlyshim reveals _find p1 vs _find_all p2 mismatchforEuclidean andk1squeezedshapeerror.','OriginalPandasappendcallfails modernPandas; realdhashflatdifferentpixelscanagree, noidentity proof.','ExplicitcopyIOfailure swallowedbyoriginalbackuphelper, thennewlyownedoriginaldeleted; no externaldata actions.'],'limits':runs[0]['limits'],'remaining':['Readremaining5textblobs including Versioneer andlicense','Visuallyreview all540datasetimages; retainperfilepins andcoverage','Original2019dependency/upstreamsuite/fullCLI/multiprocessing/platform/corpusquality/performance unqualified','No Rootproductionadoption ornewlibraryspeedup']},indent=2)+'\n')
p=D/'reviews/041.json';(D/'near41-prior-registry-20261015.json').write_bytes(p.read_bytes());reg=json.loads(p.read_text());assert not reg.get('full_repository_read',False);reg.setdefault('incremental_source_reviews',[]).append({'audit':str(out.relative_to(R))});p.write_text(json.dumps(reg,indent=2)+'\n');print(json.dumps({'status':'verified_partial','controls':12,'runs':2,'read_text':44,'remaining_text':5,'unviewed_images':540,'dependency_payloads':len(payloads)}))
