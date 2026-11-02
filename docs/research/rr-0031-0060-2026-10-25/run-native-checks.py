"""Bounded CPU correctness suites for this batch; no GPU or foreign hardware."""
import importlib.util,json,os,re
from pathlib import Path
root=Path.cwd();out=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('gate',root/'scripts/qualify-viewer-storage-gpu.py');gate=importlib.util.module_from_spec(spec);spec.loader.exec_module(gate)
suites=[('texture','rrrah-texture',''),('mesh','rrrah-mesh',''),('scientific','rrrah-scientific',''),('develop','rrrah-develop',''),('vfx','rrrah-vfx',''),('layered','rrrah-layered',''),('animation','rrrah-animation',''),('cr2','rrrah-raw','camtiff::cr2'),('mng','rrrah-decode','mng::'),('heif','rrrah-decode','heif::'),('wmf','rrrah-vector','wmf::'),('pdf','rrrah-vector','pdf::'),('cache-swap','rrrah-cache','swap::'),('dedup-model','rrrah-dedup','model_banks::'),('dedup-gradient','rrrah-dedup','gradient::'),('dedup-anchor','rrrah-dedup','anchor_rank'),('dedup-collection','rrrah-dedup','local_collection::'),('gallery','rrrah','gallery::tests::')]
results=[]
for name,package,selection in suites:
 command=['cargo','test','-p',package]+(['--bin','rrrah']if package=='rrrah'else['--lib'])+([selection]if selection else[])+['--','--show-output','--test-threads=1']
 with(out/(name+'-tests.log')).open('w')as stream:code,timeout=gate.run_suite(command,stream,os.environ.copy(),240)
 text=(out/(name+'-tests.log')).read_text();m=re.findall(r'test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored;',text)
 results.append({'name':name,'command':command,'exit_code':code,'timed_out':timeout,'summaries':m,'executed_nonzero':bool(m and int(m[-1][1])>0)})
 (out/'native-results.json').write_text(json.dumps(results,indent=2)+'\n')
