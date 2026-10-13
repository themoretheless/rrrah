#!/usr/bin/env python3
from pathlib import Path
import json,subprocess,hashlib,struct,zlib,os,csv
R=Path(__file__).resolve().parents[1];D=R/'docs/research/dedup-500';T=Path('/Users/themoretheless/.codex/tmp');B=D/'image-find-build-20261013.json';b=json.loads(B.read_text());assert b['status']=='built_pending_controls' and b['returncode']==0 and b['source_unchanged'];F=T/'rrrah-image-find-controls-20261013';src=T/'rrrah-image-find-target-20261013/debug/image-find';exe=T/'rrrah-image-find-qualified-20261013';assert not exe.exists();subprocess.run(['cp','-cp',str(src),str(exe)],check=True);sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();assert sha(src)==sha(exe);P=D/'image-find-cli-controls-20261013.json';assert not P.exists();root=F/'cases';assert not root.exists();root.mkdir();rows=[];fixtures={}
def png(value,alpha=False,pattern=False):
 channels=4 if alpha else 3;scan=b''
 for y in range(8):
  pixels=b''.join(bytes([255 if pattern and x%2 else value]*3+([0] if alpha else [])) for x in range(8));scan+=b'\0'+pixels
 def chunk(kind,data):return struct.pack('>I',len(data))+kind+data+struct.pack('>I',zlib.crc32(kind+data))
 return b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',8,8,8,6 if alpha else 2,0,0,0))+chunk(b'IDAT',zlib.compress(scan))+chunk(b'IEND',b'')
def put(path,data):path.parent.mkdir(parents=True,exist_ok=True);path.write_bytes(data);fixtures[str(path)]=sha(path)
labels=['uniform_black_white_collision','empty_references_threshold0','mixed_case_extension_skipped','corrupt_wanted_succeeds','corrupt_search_succeeds_with_valid_match','unescaped_comma_csv','parent_csv_overwritten','copy_error_still_success','out_of_range_threshold150','invalid_threshold_defaults90','png_content_in_jpg_extension','flat_scan_ignores_nested_file','transparent_hidden_rgb_changes_hash']
for i,label in enumerate(labels):
 C=root/('%02d'%i);work=C/'work';wanted=C/'wanted';search=C/'search';work.mkdir(parents=True);wanted.mkdir();search.mkdir();threshold='100';dest=None
 if label=='uniform_black_white_collision':put(wanted/'black.png',png(0));put(search/'white.png',png(255))
 elif label=='empty_references_threshold0':put(search/'query.png',png(0));threshold='0'
 elif label=='mixed_case_extension_skipped':put(wanted/'reference.png',png(0));put(search/'query.PnG',png(0))
 elif label=='corrupt_wanted_succeeds':put(wanted/'bad.png',b'not an image');put(search/'query.png',png(0));threshold='90'
 elif label=='corrupt_search_succeeds_with_valid_match':put(wanted/'reference.png',png(0));put(search/'bad.png',b'not an image');put(search/'query.png',png(0))
 elif label=='unescaped_comma_csv':put(wanted/'reference.png',png(0));put(search/'a,b.png',png(0))
 elif label=='parent_csv_overwritten':put(wanted/'reference.png',png(0));put(search/'query.png',png(0));(C/'found.csv').write_text('dedicated control sentinel\n')
 elif label=='copy_error_still_success':put(wanted/'reference.png',png(0));put(search/'query.png',png(0));dest=C/'dest';(dest/'query.png').mkdir(parents=True)
 elif label=='out_of_range_threshold150':put(wanted/'reference.png',png(0));put(search/'query.png',png(0));threshold='150'
 elif label=='invalid_threshold_defaults90':put(wanted/'reference.png',png(0));put(search/'query.png',png(0));threshold='invalid'
 elif label=='png_content_in_jpg_extension':put(wanted/'reference.png',png(0));put(search/'query.jpg',png(0))
 elif label=='flat_scan_ignores_nested_file':put(wanted/'reference.png',png(0));put(search/'nested/query.png',png(0))
 else:put(wanted/'transparent.png',png(0,alpha=True));put(search/'transparent_other_rgb.png',png(0,alpha=True,pattern=True))
 cmd=[str(exe),str(wanted),str(search),threshold]+([str(dest)] if dest else []);env=os.environ.copy();env['RAYON_NUM_THREADS']='2';run=subprocess.run(cmd,cwd=work,env=env,capture_output=True,text=True,timeout=20);out=C/'stdout';err=C/'stderr';out.write_text(run.stdout);err.write_text(run.stderr);assert run.returncode==0,(label,run.stderr);csvpath=C/'found.csv';assert csvpath.is_file() and not (work/'found.csv').exists();records=list(csv.reader(csvpath.open()));assert records[0]==['id','file','path','similarity'];n=len(records)-1
 expected={'uniform_black_white_collision':1,'empty_references_threshold0':1,'mixed_case_extension_skipped':0,'corrupt_wanted_succeeds':0,'corrupt_search_succeeds_with_valid_match':1,'unescaped_comma_csv':1,'parent_csv_overwritten':1,'copy_error_still_success':1,'out_of_range_threshold150':0,'invalid_threshold_defaults90':1,'png_content_in_jpg_extension':1,'flat_scan_ignores_nested_file':0,'transparent_hidden_rgb_changes_hash':0}[label];assert n==expected,(label,records)
 if label=='uniform_black_white_collision':assert records[1][-1]=='1.0000'
 if label=='empty_references_threshold0':assert records[1][-1]=='0.0000' and 'Loaded 0 wanted image hashes' in run.stdout
 if label in ['corrupt_wanted_succeeds','corrupt_search_succeeds_with_valid_match']:assert 'Failed to parse file' in run.stderr
 if label=='unescaped_comma_csv':assert len(records[1])==7
 if label=='parent_csv_overwritten':assert 'dedicated control sentinel' not in csvpath.read_text()
 if label=='copy_error_still_success':assert 'Failed to copy file' in run.stderr and (dest/'query.png').is_dir()
 if label=='out_of_range_threshold150':assert 'Using threshold: 1.50' in run.stdout
 if label=='invalid_threshold_defaults90':assert 'Using threshold: 0.90' in run.stdout and not run.stderr
 rows.append(dict(label=label,command=cmd,cwd=str(work),environment={'RAYON_NUM_THREADS':'2'},returncode=run.returncode,stdout=str(out),stdout_sha256=sha(out),stderr=str(err),stderr_sha256=sha(err),csv=str(csvpath),csv_sha256=sha(csvpath),parsed_csv_rows=records,expected_match_rows=expected))
assert all(sha(k)==h for k,h in fixtures.items());P.write_text(json.dumps(dict(status='complete13actual_cli_controls_pending_audit',pins={str(p):sha(p) for p in [B,Path(__file__),exe]},executable=str(exe),fixtures=fixtures,rows=rows,scope='Original locked CLI binary, isolated per-case current directories and outputs; no production source changes. Thirteen authored CLI observations/counterexamples, not upstream tests or generic copy precision. Original CSV writes to parent directory, so all controls execute below dedicated case directories. Source/library format/platform/scale/lifecycle/resource gaps remain separate.'),indent=2)+'\n');print(P)
