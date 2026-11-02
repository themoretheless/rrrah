import os,subprocess,json,hashlib,re,tempfile,shutil,filecmp,time
from pathlib import Path
root=Path("docs/research/dedup-500/sony-display-matrix-20261026-v1");binary=Path("target/release/examples/raw_raster_probe")
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
raw=Path.home()/".cache/rrrah-dedup/raw-corpus/2414.arw";tiff=Path.home()/".cache/rrrah-dedup/independent-rendered-20261026/2414.tiff";bs=sha(binary)
with tempfile.TemporaryDirectory(prefix="rrrah-repeated-content-") as tmp:
 copy=Path(tmp)/"copy.tiff";shutil.copyfile(tiff,copy);assert filecmp.cmp(tiff,copy,shallow=False)
 inputs=[raw,tiff,copy];before=[sha(p) for p in inputs]
 env={k:v for k,v in os.environ.items() if not k.startswith("RRRAH_")};env.update(RRRAH_MANAGED_BUDGET_MIB="1000",RRRAH_RAW_RECIPE="bilinear",RRRAH_AUTO_WHITE_ANCHOR="1",RRRAH_VERIFY_COLLECTION="1",RRRAH_COLLECTION_PATHS="\n".join(str(p) for p in inputs))
 log=root/"real-sony-repeated-content-20261026.log";start=time.monotonic()
 with log.open("xb") as f:r=subprocess.run([str(binary),str(raw),str(tiff),"--verify-spatial-display"],env=env,stdout=f,stderr=subprocess.STDOUT)
 s=log.read_text();candidates=[(int(a),int(b)) for a,b in re.findall(r"pair=\((\d+), (\d+)\) candidate=true",s)];stable=before==[sha(p) for p in inputs] and bs==sha(binary);clean=all(x in s for x in ["file_issues=[]","pair_issues=[]","source_issues=[]"])
 report=dict(exit_code=r.returncode,seconds=time.monotonic()-start,binary_sha256=bs,inputs={str(p):v for p,v in zip(inputs,before)},stable=stable,candidates=candidates,issues_empty=clean,managed_released="managed_used=0 " in s,log=str(log),log_sha256=sha(log),label_control="TIFF copy independently byte-compared with filecmp(shallow=False); copy created in owned temporary directory, removed after this run.")
 (root/"real-sony-repeated-content-20261026.json").write_text(json.dumps(report,indent=2)+"\n")
 print(r.returncode,candidates,clean,flush=True)
 assert r.returncode==0 and stable and clean and report["managed_released"] and candidates==[(1,2),(1,3),(2,3)]
