import os,json,subprocess,hashlib
from pathlib import Path
out=Path('/Users/themoretheless/Documents/Sources/rrrah/docs/research/sustained-swap-current-2026-10-16');out.mkdir(exist_ok=True)
binary=Path('/tmp/rrrah-alpha-isolated-target-20261016/debug/deps/sustained_swap-adcaccbc44b87a76')
names=['sustained_large_keyspace_delayed_swap_pressure_and_retry','sustained_byte_cap_evicts_without_count_or_ttl_pressure','sustained_count_cap_evicts_without_byte_or_ttl_pressure','sustained_float_hdr_delayed_swap_pressure_cancel_and_retry']
r={'status':'running','binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'seconds_per_test':60,'scope':'real local files with injected2ms/64KiB delays; not physical slow disk or GPU','cases':[]}
for name in names:
 r['current']=name;(out/'result.json').write_text(json.dumps(r,indent=2)+'\n')
 with (out/(name+'.log')).open('w') as log:
  code=subprocess.run([str(binary),'--ignored','--exact',name,'--nocapture'],env=dict(os.environ,RRRAH_STRESS_SECONDS='60'),stdout=log,stderr=subprocess.STDOUT).returncode
 r['cases'].append({'test':name,'exit_code':code});(out/'result.json').write_text(json.dumps(r,indent=2)+'\n');print(name,code,flush=True)
 if code:r['status']='failed';break
else:r['status']='passed'
r.pop('current',None);(out/'result.json').write_text(json.dumps(r,indent=2)+'\n')
