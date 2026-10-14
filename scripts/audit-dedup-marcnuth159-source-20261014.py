#!/usr/bin/env python3
from pathlib import Path
import hashlib,json,ast,shutil,sys
from packaging.requirements import Requirement,InvalidRequirement
import packaging
R=Path(__file__).resolve().parents[1];D=R/'docs/research';E=D/'dedup-500/evidence/Marcnuth--deduplication';P=D/'dedup-marcnuth159-controls-20261014.json';d=json.loads(P.read_text());sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert d['status']=='passed_bounded_original_core_controls' and len(d['controls'])==31 and all(x['passed']for x in d['controls'])
assert all(sha(p)==h for p,h in d['source_pins'].items()) and sha(d['original_test_log'])==d['original_test_log_sha256']
producer=R/'scripts/check-dedup-marcnuth159-20261014.py';assert sha(producer)==d['producer_sha256']
review_path=D/'dedup-500/reviews/159.json';review=json.loads(review_path.read_text());assert review['repository']=='Marcnuth/deduplication'
tree=json.loads((E/'tree.json').read_text());assert not tree['truncated'];blobs=[x for x in tree['tree']if x['type']=='blob'];assert len(blobs)==12
mapping={x['path']:x['local']for x in review['available_source_evidence']+review['available_readme_license_evidence']}
mapping.update({'.gitignore':'07-gitignore','requirements.txt':'08-requirements.txt','setup.py':'09-setup.py','tests/data/wiki_nlp.txt':'10-wiki_nlp.txt','tests/test_nlp.py':'11-test_nlp.py'})
files=[]
for row in blobs:
    p=E/mapping[row['path']];data=p.read_bytes();assert hashlib.sha1(b'blob '+str(len(data)).encode()+b'\0'+data).hexdigest()==row['sha']
    text=data.decode();files.append(dict(path=row['path'],local=p.name,sha256=sha(p),git_blob_sha=row['sha'],bytes=len(data),lines=len(text.splitlines())))
    if p.suffix=='.py':ast.parse(text,filename=row['path'])
requirement=(E/'08-requirements.txt').read_text().strip()
try:Requirement(requirement)
except InvalidRequirement as e:requirement_error=dict(type=type(e).__name__,message=str(e))
else:raise AssertionError('Original quoted version requirement unexpectedly valid')
F=D/'dedup-marcnuth159-controls-20261014';F.mkdir(exist_ok=False)
original_log=F/'original-demo-tests.log';shutil.copyfile(d['original_test_log'],original_log);assert sha(original_log)==d['original_test_log_sha256']
O=D/'dedup-500/marcnuth159-complete-source-review-20261014.json';assert not O.exists()
report=dict(status='complete12file_source_review_31core_controls_and_packaging_check',index=159,repository=review['repository'],commit=review['commit'],whole_repo_complete=True,whole_runtime_qualified=False,fully_read_files=files,license='Full Apache2text read; no foreign source adopted.',controls=str(P.relative_to(R)),controls_sha256=sha(P),control_producer=str(producer.relative_to(R)),control_producer_sha256=sha(producer),auditor_sha256=sha(__file__),requirement_validation=dict(requirement=requirement,error=requirement_error,packaging_version=packaging.__version__,runtime_python=sys.version),original_demo_tests=d['original_demo_tests'],retained_demo_log=str(original_log.relative_to(R)),retained_demo_log_sha256=sha(original_log),decision='Keep signed majority representation as conceptual reference only; reject this implementation for production exact/pixel/native identity or all-case dedup.',findings=['Leading zero MD5bits padded with neutral0rather than signed-1change actual128bit majority result versus independent fixed-width oracle.','Empty or entirely filtered text collapses to one all-zero fingerprint; bag order and uniform repetition are intentionally lost and cannot prove file identity.','Longest-sentence selection discards changed shorter sentences; zero/negative n_longest silently discards content.','Malformed block counts produce ZeroDivisionError, AssertionError or ValueError; fractional and boolean counts accepted with surprising output lengths.','Original README import calls modules as functions and raises TypeError on unchanged package API.','Original requirement spacy>=\'2.1.4\' rejected by actual installed packaging.Requirement parser; no setup/install attempted.','Language model loaded on each tokenization and sentencizer call; no work/cancellation/memory or model-generation cache policy.','Four original test functions print output and contain no assertions; controlled fixture dependency allows execution but does not qualify real NLP/model accuracy.','No file scan, Hamming verification/publication, visual/image decoder or pixel identity implementation present. Prior registry broad-media README phrasing corrected: actual README discusses text documents.'],limits='All12original tree blobs stored/read/pinned, including20KiBfixture, complete source/test/config/license. Real NumPy/MD5core, controlled spaCy Token/Pipeline adapter; unadapted runtime import fails missing spaCy. No real spaCy/English model execution, dependency installation, semantic quality, platform/format/resource or all500completion claim.')
O.write_text(json.dumps(report,indent=2)+'\n')
archive=D/'dedup-500/marcnuth159-prior-registry-20261014.json';assert not archive.exists();shutil.copyfile(review_path,archive)
for file in files:
    if file['path']not in {x['path']for x in review['available_source_evidence']+review['available_readme_license_evidence']}:
        review['available_source_evidence'].append(dict(path=file['path'],local=file['local'],url=f"https://github.com/{review['repository']}/blob/{review['commit']}/{file['path']}",sha256=file['sha256'],truncated=False,lines=file['lines'],source=file['path'].endswith('.py'),test=file['path'].startswith('tests/')))
review.update(review_scope='Complete12blob pinned source/config/license/test/text-fixture reading;31original core controls with real NumPy/MD5and explicit controlled spaCy adapter, quoted requirement parsing, four adapted print-only demos. No real learned NLP/model/runtime/platform or full500qualification.',finding='MD5leading-zero majority bug; empty/filtered and longest-sentence collisions; invalid quoted dependency and README module-call API;31bounded core controls, no production adoption.',full_repository_review_complete=True,whole_repo_complete=True,whole_runtime_qualified=False,complete_source_review=str(O.relative_to(D/'dedup-500')),prior_registry_review=str(archive.relative_to(D/'dedup-500')),validated_original_demo_functions=4,upstream_tests_run=False,upstream_test_execution_scope='Four original print-only demos executed with controlled dependency adapter, not unadapted upstream runtime qualification.')
review_path.write_text(json.dumps(review,indent=2)+'\n')
index=D/'dedup-500/REPOSITORIES.md';lines=index.read_text().splitlines();prefix='| 159 |';matches=[i for i,l in enumerate(lines)if l.startswith(prefix)];assert len(matches)==1;i=matches[0];parts=lines[i].split('|');parts[-2]=' '+review['finding']+' ';lines[i]='|'.join(parts);index.write_text('\n'.join(lines)+'\n')
print(O)
