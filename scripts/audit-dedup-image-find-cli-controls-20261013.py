#!/usr/bin/env python3
from pathlib import Path
import json,hashlib,csv,struct,zlib
R=Path(__file__).resolve().parents[1];D=R/'docs/research/dedup-500';P=D/'image-find-cli-controls-20261013.json';d=json.loads(P.read_text());M=D/'image-find-full-source-20261013.json';m=json.loads(M.read_text());S=Path(m['source']);sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();assert d['status']=='complete13actual_cli_controls_pending_audit';assert all(sha(k)==h for k,h in d['pins'].items()) and all(sha(k)==h for k,h in d['fixtures'].items());t=json.loads((D/'evidence/ImShyMike--image-find/tree.json').read_text());assert not t['truncated'];tree={e['path']:e for e in t['tree'] if e['type']=='blob'};assert len(tree)==len(m['files'])==10;assert set(tree)=={str(p.relative_to(S)) for p in S.rglob('*') if p.is_file()};B=D/'image-find-build-20261013.json';b=json.loads(B.read_text());assert b['status']=='built_pending_controls' and b['returncode']==0 and b['source_unchanged'];assert sha(b['log'])==b['log_sha256'];F=Path(b['source'])
for row in m['files']:
 p=S/row['path'];raw=p.read_bytes();assert hashlib.sha1(b'blob '+str(len(raw)).encode()+b'\0'+raw).hexdigest()==tree[row['path']]['sha']==row['git_blob'];assert sha(p)==sha(F/row['path'])==row['sha256']
assert len(d['rows'])==len({r['label'] for r in d['rows']})==13
for r in d['rows']:
 for key in ['stdout','stderr','csv']:assert sha(r[key])==r[key+'_sha256']
 records=list(csv.reader(Path(r['csv']).open()));assert records==r['parsed_csv_rows'] and len(records)-1==r['expected_match_rows'];assert records[0]==['id','file','path','similarity'];assert r['returncode']==0 and r['command'][0]==d['executable'];assert Path(r['csv']).parent==Path(r['cwd']).parent and not (Path(r['cwd'])/'found.csv').exists()
rows={r['label']:r for r in d['rows']};assert rows['uniform_black_white_collision']['parsed_csv_rows'][1][-1]=='1.0000';assert rows['empty_references_threshold0']['parsed_csv_rows'][1][-1]=='0.0000';assert len(rows['unescaped_comma_csv']['parsed_csv_rows'][1])==7;assert rows['transparent_hidden_rgb_changes_hash']['expected_match_rows']==0
for label in ['corrupt_wanted_succeeds','corrupt_search_succeeds_with_valid_match']:assert 'Failed to parse file' in Path(rows[label]['stderr']).read_text()
assert 'Failed to copy file' in Path(rows['copy_error_still_success']['stderr']).read_text();assert (Path(rows['copy_error_still_success']['csv']).parent/'dest/query.png').is_dir()
# Independently decode the synthetic lossless PNG streams: check dimensions,
# CRCs, filter0 scanlines, and pixel distinction/equal-alpha fixture assertions.
pixels={}
for name in d['fixtures']:
 p=Path(name);raw=p.read_bytes()
 if not raw.startswith(b'\x89PNG\r\n\x1a\n'):continue
 at=8;compressed=b'';header=None
 while at<len(raw):
  n=struct.unpack('>I',raw[at:at+4])[0];kind=raw[at+4:at+8];data=raw[at+8:at+8+n];crc=struct.unpack('>I',raw[at+8+n:at+12+n])[0];assert zlib.crc32(kind+data)==crc;at+=12+n
  if kind==b'IHDR':header=struct.unpack('>IIBBBBB',data)
  if kind==b'IDAT':compressed+=data
 assert header[:3]==(8,8,8) and header[3] in [2,6];channels=3 if header[3]==2 else 4;scan=zlib.decompress(compressed);assert len(scan)==8*(1+8*channels);body=b''
 for y in range(8):assert scan[y*(1+8*channels)]==0;body+=scan[y*(1+8*channels)+1:(y+1)*(1+8*channels)]
 pixels[name]=(channels,body)
collision=rows['uniform_black_white_collision'];base=Path(collision['cwd']).parent;assert set(pixels[str(base/'wanted/black.png')][1])=={0};assert set(pixels[str(base/'search/white.png')][1])=={255};base=Path(rows['transparent_hidden_rgb_changes_hash']['cwd']).parent;a=pixels[str(base/'wanted/transparent.png')];bpx=pixels[str(base/'search/transparent_other_rgb.png')];assert a[0]==bpx[0]==4 and a[1]!=bpx[1] and set(a[1][3::4])==set(bpx[1][3::4])=={0}
V=D/'image-find-complete-source-review-20261013.json';v=json.loads(V.read_text());Q=D/'image-find-lock-and-gif-review-20261013.json';q=json.loads(Q.read_text());assert len(q['lock_packages'])==135 and len(q['resolved_dependency_edges'])==247 and len(q['gif_frames'])==188 and q['gif_selected_visual_frame_indices']==[0,62,125,187];assert set(v['direct_complete_text_reads'])|{'Cargo.lock','assets/demo.gif'}==set(tree);O=D/'image-find-complete-runtime-audit-20261013.json';assert not O.exists();O.write_text(json.dumps(dict(status='verified10entry_review_and13actual_locked_cli_controls',repository=m['repository'],commit=m['commit'],pins={str(p):sha(p) for p in [P,M,B,V,Q,Path(q['visual_contact_sheet']),Path(__file__)]},direct_complete_text_reads=8,structured_lock_packages=135,structured_lock_edges=247,structured_gif_frames=188,visually_inspected_gif_frames=[0,62,125,187],authored_cli_controls=13,original_upstream_tests_run=False,scope='All ten original entries reviewed with explicit direct/structured/visual scopes, not ten manual text reads or all188frames visually inspected. Complete Git blob/source-copy/compiler/executable/input/log/CSV pins and independent synthetic PNG CRC/pixel checks. Actual original CLI binary; no algorithm/source adapters, code adoption, all-platform/scale/resource/lifecycle/format or general copy-precision claim. A hash collision is candidate evidence, not identity; zero reference/error/CSV/copy behaviors make this CLI unsuitable as authoritative duplicate confirmation.'),indent=2)+'\n');print(O)
