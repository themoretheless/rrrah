from pathlib import Path
import subprocess,json,hashlib,re
root=Path("docs/research/dedup-500/evidence/autobrr--upbrr");binary=Path("target/release/examples/selected_frame_compare")
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
bs=sha(binary);rows=[]
for label,ico in [("documentation","documentation/static/img/favicon.ico"),("webui","webui/public/favicon.ico")]:
 for index,size,expected in [(0,48,True),(1,32,True),(2,16,True),(0,16,False),(3,48,None)]:
  left=root/"remaining-pinned"/ico;right=root/"ico-frame-extracts-20261026"/(label+"-"+str(size)+"x"+str(size)+".png");inputs={str(p):sha(p) for p in [left,right]}
  result=subprocess.run([str(binary),str(left),str(right),str(index),"0"],capture_output=True,text=True)
  log=root/("selected-frame-"+label+"-index"+str(index)+"-png"+str(size)+"-20261026.log");assert not log.exists();log.write_text(result.stdout+result.stderr)
  assert bs==sha(binary) and inputs=={str(p):sha(p) for p in [left,right]}
  row=dict(label=label,index=index,png_size=size,expected_equal=expected,exit_code=result.returncode,log=str(log),log_sha256=sha(log),inputs=inputs);rows.append(row)
  (root/"ico-selected-frame-qualification-20261026.json").write_text(json.dumps(dict(binary_sha256=bs,rows=rows),indent=2)+"\n")
  if expected is None:assert result.returncode!=0 and "UnsupportedImageIndex" in result.stderr
  else:
   assert result.returncode==0 and "selected_frame_equal="+str(expected).lower() in result.stdout and "digest_equal="+str(expected).lower() in result.stdout and "left_count=3" in result.stdout and "right_count=1" in result.stdout and "managed_used=0 " in result.stdout
  print(label,index,size,result.returncode,expected,flush=True)
