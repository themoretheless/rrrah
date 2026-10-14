#!/usr/bin/env python3
from pathlib import Path
import json,hashlib,subprocess,re
R=Path(__file__).resolve().parents[1];D=R/'docs/research';sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();P=D/'dedup-gradient-lanes16-public-20261014.json';p=json.loads(P.read_text());lib=Path('/private/tmp/rrrah-gradient-lanes16-public-20261014/candidate/librrrah_dedup.rlib');assert sha(lib)==p['pins'][str(lib)];assert all(sha(k)==h for k,h in p['dependency_pins'].items());prior=json.loads((D/'dedup-gradient-unroll4-managed-focused-v2-20261014.json').read_text());deps=[]
for k,v in zip(prior['command'],prior['command'][1:]):
 if k in ['-L','--extern']and not v.startswith('rrrah_core='):deps.extend([k,v])
core=next(v for k,v in zip(prior['command'],prior['command'][1:])if k=='--extern'and v.startswith('rrrah_core='));deps+=['--extern',core,'--extern','rrrah_dedup='+str(lib)];F=Path('/private/tmp/rrrah-gradient-distinct-raster-guard-20261014');F.mkdir(exist_ok=False);source=R/'crates/rrrah-dedup/tests/gradient_distinct_locations.rs';source_sha=sha(source);exe=F/'tests';cmd=['rustc','+stable','--edition=2024','--test',str(source),'--cfg','feature="raster"','-C','opt-level=1',*deps,'-o',str(exe)];O=D/'dedup-gradient-distinct-raster-guard-20261014.json';assert not O.exists();d=dict(status='building',command=cmd,source=str(source),source_sha256=source_sha,qualified_raster_library=str(lib),qualified_raster_library_sha256=sha(lib),prior_public_report_sha256=sha(P),dependency_pins=p['dependency_pins'],producer_sha256=sha(__file__),scope='Actual current integration test source with raster enabled against retained independently qualified Mac16candidate managed raster library and real cached core dependency. Executes formerly unguardedmanagedtest body unchanged. Not currentcore/workspace/rasterCargo/full-feature proof; no skips or assertions removed.')
def save():
 t=O.with_suffix('.tmp');t.write_text(json.dumps(d,indent=2)+'\n');t.replace(O)
save()
for label,command in [('build',cmd),('inventory',[str(exe),'--list']),('tests',[str(exe),'--test-threads','2'])]:
 with (F/f'{label}.log').open('x')as log:
  c=subprocess.Popen(command,cwd=R,stdout=log,stderr=subprocess.STDOUT);d.update(status=label,child_pid=c.pid);save();rc=c.wait()
 d[label+'_returncode']=rc;save();assert rc==0
s=(F/'tests.log').read_text();names=set(re.findall(r'^(.+): test$',(F/'inventory.log').read_text(),re.M));results=dict(re.findall(r'^test (.+) \.\.\. (ok|FAILED|ignored)$',s,re.M));assert names==set(results)and all(v=='ok'for v in results.values())and results['managed_distinct_match_admission_last_owner_and_cancellation']=='ok';assert sha(source)==source_sha and sha(lib)==d['qualified_raster_library_sha256'];d.update(status='verified_current_test_source_on_qualified_raster_library',passed=len(names),failed=0,ignored=0,pins={str(v):sha(v)for v in [exe,F/'build.log',F/'inventory.log',F/'tests.log']});save();print(len(names),'raster integration tests passed')
