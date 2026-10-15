"""Read-only observation of the already running owned opt3 build; never restarts."""
from pathlib import Path
import datetime,hashlib,json,subprocess
R=Path(__file__).resolve().parents[1];D=R/'docs/research';h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();P=D/'dedup-durable-actual-benchmark-v5-protocol-20261016.json';p=json.loads(P.read_text());work=Path(p['work']);report=D/'dedup-durable-actual-benchmark-v5-build-20261016.json'
rows=[]
for pid in [87411,87426,88197]:
 result=subprocess.run(['ps','-p',str(pid),'-o','pid=,ppid=,etime=,comm='],capture_output=True,text=True)
 if result.returncode==0:
  parts=result.stdout.strip().split(None,3);assert len(parts)==4;rows.append({'pid':int(parts[0]),'parent':int(parts[1]),'elapsed':parts[2],'executable':parts[3]})
parent=next((x for x in rows if x['pid']==87411),None);cargo=next((x for x in rows if x['pid']==87426),None);rustc=next((x for x in rows if x['pid']==88197),None)
if parent:
 command=subprocess.check_output(['ps','-p','87411','-o','args='],text=True);assert 'scripts/build-dedup-durable-actual-benchmark-v5-20261016.py' in command
if cargo:assert cargo['parent']==87411 and Path(cargo['executable']).name=='cargo'
if rustc:assert rustc['parent']==87426 and Path(rustc['executable']).name=='rustc'
logs={str(x.relative_to(R)):{'sha256':h(x),'bytes':x.stat().st_size} for x in [work/'cargo-json.log',work/'cargo-stderr.log'] if x.is_file()}
d={'status':'terminal_build_report_available' if report.is_file() else 'verified_owned_build_live' if parent and cargo else 'owned_build_handles_missing_requires_diagnosis','observed_at_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'rows':rows,'logs_at_observation':logs,'protocol_sha256':h(P),'inspector_sha256':h(__file__),'scope':'Specific existing producer/cargo/rustc PID parentage and producer command verified now. Missing historical rustc alone not terminal. No restart/timing/adoption.'}
out=D/'dedup-durable-opt3-build-observation-v5-20261016.json';assert not out.exists();out.write_text(json.dumps(d,indent=2)+'\n');print(json.dumps(d))
