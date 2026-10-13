#!/usr/bin/env python3
import hashlib,json,re
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 out=D/'dedup-loftr-column-softmax-memory-audit-20261012.json';assert not out.exists();rp=D/'dedup-loftr-column-softmax-memory-20261012.json';r=json.loads(rp.read_text());assert r['status']=='complete_two_process_softmax_memory_comparison'and all(sha(p)==v for p,v in r['pins'].items());a,b=r['results'];assert [a['mode'],b['mode']]==['original','aligned'];assert r['rss_delta_bytes']==b['rss_bytes']-a['rss_bytes']and b['rss_bytes']>=a['rss_bytes']
 for row in [a,b]:assert row['returncode']==0 and row['rss_bytes']==int(re.search(r'(\d+)\s+maximum resident set size',row['stderr'])[1])and '1 passed; 0 failed'in row['stdout']
 assert (a['first'],a['sum'])==(b['first'],b['sum']);mp=D/'dedup-loftr-column-softmax-aligned-candidate-20261012.json';m=json.loads(mp.read_text());assert m['tests']==10 and all(sha(p)==v for p,v in m['pins'].items());assert all(sha(Path(m['candidate'])/k)==v for k,v in m['source_file_hashes'].items());old=D/'dedup-loftr-column-softmax-candidate-20261012.json';v=json.loads(old.read_text());assert v['status']=='rejected_exact_parity_failure_not_adopted'and all(sha(R/'vendor/loftr'/k)==h for k,h in v['baseline_hashes'].items())
 out.write_text(json.dumps({'status':'verified_no_measured_rss_benefit_candidate_not_adopted','original_rss_bytes':a['rss_bytes'],'aligned_rss_bytes':b['rss_bytes'],'rss_delta_bytes':r['rss_delta_bytes'],'unit_tests':10,'pins':{str(p):sha(p)for p in [rp,mp,old,Path(__file__)]},'scope':'Single separate-process synthetic6400x6400 CPU normalization control, equal sampled output/sum plus10unit exact/autograd/cancellation tests. Original435699712bytes versus aligned438845440bytes RSS: no memory improvement demonstrated. No root production change or whole-model resource bound; not a statistically established universal regression. Dense confidence/similarity storage remains.'},indent=2)+'\n');print('Verified no measured RSS benefit; candidate not adopted')
if __name__=='__main__':main()
