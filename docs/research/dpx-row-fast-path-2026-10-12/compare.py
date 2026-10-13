from pathlib import Path
import subprocess,json,hashlib
p=Path(__file__).resolve().parent
executables={'before':'/tmp/rrrah-dpx-before-fast-path-20261012','after':str(p.parents[2]/'target/debug/examples/raster_load_bench')}
source='/tmp/rrrah-dpx-12mp-current-20261012.dpx'
results=[]
for index,version in enumerate(['before','after','after','before']):
 result=subprocess.run([executables[version],source,'0','11','--managed-srgb'],text=True,capture_output=True,check=True)
 (p/f'{index}-{version}.json').write_text(result.stdout)
 (p/f'{index}-{version}-memory.log').write_text(result.stderr)
 row=json.loads(result.stdout);row['version']=version;results.append(row)
assert len({r['display_blake3'] for r in results})==1
report={'order':['before','after','after','before'],'runs':results,'binary_sha256':{k:hashlib.sha256(Path(v).read_bytes()).hexdigest() for k,v in executables.items()},'source_sha256':hashlib.sha256(Path(source).read_bytes()).hexdigest(),'scope':'dev opt-level=1 workspace/3 dependencies; 3 warmups then 11 iterations per run; warm filesystem, CPU decode/preparation, shared host'}
(p/'paired.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps([{'version':r['version'],'load_p50_ms':r['load_p50_ms'],'ready_p50_ms':r['cpu_ready_p50_ms']} for r in results]))
