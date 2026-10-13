#!/usr/bin/env python3
"""Record actual process RSS separately from managed payload admission."""
import hashlib,json,re
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 out=D/'dedup-cpu-resource-gap-20261012.json';assert not out.exists()
 rp=D/'dedup-disc21-exposure-recovery-collection-20261012.json';ap=D/'dedup-disc21-exposure-recovery-collection-audit-20261012.json';r=json.loads(rp.read_text());a=json.loads(ap.read_text());assert a['status']=='verified_actual_cpu_three_source_exposure_recovery_collection'and all(sha(p)==v for p,v in a['pins'].items());assert all(sha(p)==v for p,v in r['pins'].items())
 assert r['status']=='complete_three_source_collection_control'and r['returncode']==0;v=json.loads(r['stdout']);rss=int(re.search(r'(\d+)\s+maximum resident set size',r['stderr'])[1]);assert v['managed_used']==0 and rss>v['managed_peak']
 source=next(Path(p)for p in r['pins']if p.endswith('/src/bin/disc21_exposure_recovery_collection.rs'));assert 'MemoryBudget::new(512 * 1024 * 1024)'in source.read_text();cap=512*1024*1024
 out.write_text(json.dumps({'status':'measured_native_cpu_rss_exceeds_managed_payload_limit','process_peak_rss_bytes':rss,'managed_peak_bytes':v['managed_peak'],'managed_cap_bytes':cap,'managed_used_after_report_drop':v['managed_used'],'rss_over_managed_cap':rss/cap,'elapsed_seconds':r['elapsed_seconds'],'pins':{str(p):sha(p)for p in [rp,ap,source,Path(__file__)]},'scope':'Actual macOS ARM CPU three-source control /usr/bin/time -l measurement. Managed payload accounting works;512MiB is not a total RSS cap. Models, tensors, provider workspace and metadata remain external. This measurement identifies a resource-admission gap; it does not measure isolated tensor peak, prove a leak or establish a universal LoFTR worst-case bound. Resource gap remains required work.'},indent=2)+'\n');print(f'CPU RSS {rss} bytes, managed peak {v["managed_peak"]} bytes, payload cap {cap} bytes')
if __name__=='__main__':main()
