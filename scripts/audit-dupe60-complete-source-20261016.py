"""Verify retained bytes supporting the completed human source/media review declaration."""
from pathlib import Path
import hashlib,json,re,tomllib
R=Path(__file__).resolve().parents[1];D=R/'docs/research/dedup-500';W=R/'.dedup-work/dupe60-20261016';h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
p=D/'dupe60-fetched-source-pins-20261016.json';a=json.loads(p.read_text());assert len(a['files'])==97 and a['verified_trees']==18
files=[]
for x in a['files']:
 f=W/'source'/x['path'];b=f.read_bytes();assert len(b)==x['bytes'] and h(f)==x['sha256'];assert hashlib.sha1(b'blob '+str(len(b)).encode()+b'\0'+b).hexdigest()==x['git_blob'];files.append({'path':x['path'],'sha256':h(f),'review':'complete text read or structured lock/SVG review' if 'content' in x else 'visual image review or complete font table inspection and mapped glyph pages'})
lock=tomllib.loads((W/'source/Cargo.lock').read_text());assert len(lock['package'])==174
for package in lock['package']:
 assert set(package)<= {'name','version','source','checksum','dependencies'}
 if 'checksum' in package:assert re.fullmatch('[0-9a-f]{64}',package['checksum'])
refs=[D/'dupe60-whole-native-partial-source-review-20261016.json',D/'dupe60-whole-cli-controls-20261016.json',D/'dupe60-benchmark-controls-20261016.json']
bench=json.loads(refs[-1].read_text());assert h(R/'scripts/qualify-dupe60-benchmark-controls-20261016.py')==bench['producer_sha256']
for ref in refs:
 for name,digest in json.loads(ref.read_text()).get('evidence_pins',{}).items():assert h(R/name)==digest,(ref,name)
media=[*sorted((W/'svg-review').glob('*.svg.full.png')),*sorted((W/'font-review').glob('*.png')),W/'font-metadata.json',W/'font-pages.json',W/'svg-render.rs',W/'svg-render']
assert len(list((W/'svg-review').glob('*.svg.full.png')))==3 and len(list((W/'font-review').glob('*.png')))==7
out=D/'dupe60-complete-source-review-20261016.json';assert not out.exists()
out.write_text(json.dumps({'status':'verified_bytes_and_completed_source_media_review_declaration','repository':'Rafaelpta/dupehound','commit':'35ea7c3743a8fb2205aaef973e67b1c6c5e41474','whole_repo_complete':True,'whole_implementation_complete':True,'files':files,'acquisition_sha256':h(p),'auditor_sha256':h(__file__),'runtime_references':{str(x.relative_to(R)):h(x) for x in refs},'visual_review_artifacts':{str(x.relative_to(R)):h(x) for x in media},'review_method':['All 90 UTF8 files read; Cargo.lock all174 package records structurally inspected, not dependency source review','All4 original PNGs visually viewed; all3 SVG geometry/text read and full intrinsic-size renders viewed','All3 bundled TTF and embedded Virgil font tables parsed; all824 mapped codepoint placements viewed across7 pages; unmapped glyph tables structurally inspected','Full98 unchanged original tests twice;8 actual CLI controls twice; original free synthetic benchmark CI twice plus5 mapping/policy controls twice'],'findings':['101-copy fingerprint postings culled, all duplicate pairs lost','Rust comment marker can misclassify ordinary functions as tests','History blob cache omits language key; max snapshots1 panics; custom history excludes ignored','Benchmark precision conditional on labelled universe; original methodology discloses this limitation','Same basename/function-name ground-truth index collision reproduced on owned two-directory fixture'],'limits':['Review completeness is a human declaration supported by byte pins, not inferred from acquisition','Published VSCode and paid-agent experiments read only, not reproduced; no paid agents or user transcript access','Code clone algorithms are references, no copied implementation or production speedup claim','Not a full third-party dependency review, all-platform qualification, or image/video accuracy campaign']},indent=2)+'\n');print('Completed dupehound source/media declaration:97 blobs,18 trees,174 lock packages')
