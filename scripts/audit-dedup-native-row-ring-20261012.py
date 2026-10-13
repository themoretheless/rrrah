#!/usr/bin/env python3
import hashlib,json,re,shutil
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 mp=D/'dedup-native-row-ring-snapshot-20261012.json';m=json.loads(mp.read_text());s=Path(m['snapshot']);assert sha(m['parent_manifest'])==m['parent_sha256'];assert all(sha(s/k)==v for k,v in m['file_hashes'].items())
 for k,v in m['file_hashes'].items():
  if k.startswith('crates/rrrah-dedup/src/') or k=='crates/rrrah-dedup/tests/gradient_candidate_smoothing.rs':assert sha(R/k)==v,k
 log=T/'rrrah-native-row-ring-raster-tests-20261012.log';text=log.read_text();names=re.findall(r'^test (\S+) \.\.\. ok$',text,re.M);assert len(names)==6 and len(set(names))==6 and '6 passed; 0 failed; 0 ignored' in text
 required={'row_ring_is_bitwise_full_two_pass_for_wraps_borders_and_hdr','row_ring_admits_only_window_scratch_and_refuses_one_byte_short','row_ring_wrap_cancellation_and_late_alpha_never_retain_partial_output'};assert required.issubset(names)
 matches=re.findall(r'Running tests/gradient_candidate_smoothing.rs \(([^)]+)\)',text);assert len(matches)==1;b=T/'rrrah-native-row-ring-raster-executable-20261012';assert not b.exists();shutil.copy2(matches[0],b)
 rp=D/'dedup-native-row-ring-memory-20261012.json';r=json.loads(rp.read_text());assert r['status']=='complete_identical_smoothing_stage_memory_control' and r['build_returncode']==0;assert all(sha(k)==v for k,v in r['pins'].items());assert sha(r['build_log'])==r['build_log_sha256'];assert [x['mode']for x in r['results']]==['baseline','ring']
 old,new=r['results'];a=json.loads(old['stdout']);c=json.loads(new['stdout']);assert a==old['evidence']and c==new['evidence'];assert a['output_digest']==c['output_digest']and a['retained']==c['retained']==50331648 and a['after_drop']==c['after_drop']==0;assert a['managed_peak']==100663296 and c['managed_peak']==50626560
 for row in [old,new]:assert row['returncode']==0 and row['rss_bytes']==int(re.search(r'(\d+)\s+maximum resident set size',row['stderr'])[1])
 assert old['rss_bytes']==152764416 and new['rss_bytes']==102727680
 # Bind the retained baseline to the whole original function, not a substitute.
 parent=json.loads(Path(m['parent_manifest']).read_text());original=(Path(parent['snapshot'])/'crates/rrrah-dedup/src/gradient_scale.rs').read_text();original=original[original.index('pub fn smooth_gradient_candidates_managed('):].replace('pub fn smooth_gradient_candidates_managed(', 'fn original_full_frame(')
 harness=T/'rrrah-native-row-ring-memory-harness-20261012/src/main.rs';assert harness.read_text().endswith(original)
 out=D/'dedup-native-row-ring-verification-20261012.json';assert not out.exists();out.write_text(json.dumps({'status':'verified_six_smoothing_controls_and_stage_rss_reduction','tests':names,'bitwise_reference_cases':35,'all_cancellation_checkpoints':272,'stage_baseline_rss_bytes':old['rss_bytes'],'stage_ring_rss_bytes':new['rss_bytes'],'stage_rss_reduction_bytes':old['rss_bytes']-new['rss_bytes'],'managed_baseline_peak_bytes':a['managed_peak'],'managed_ring_peak_bytes':c['managed_peak'],'pins':{str(p):sha(p)for p in [mp,log,b,rp,harness,Path(__file__)]},'scope':'Current-identical smoothing implementation; independent full two-pass bitwise oracle on35 shapes/radii/HDR cases, exact window/output admission, one-byte/work short refusals, all272 cancellation checkpoints, late opacity and last-owner release. Real separate-process 2048x1536 radius4 smoothing stage has identical full output digest and lower RSS. Whole native portfolio control and broader integration gate remain pending; no process-wide cap or universal speedup.'},indent=2)+'\n');print('Verified six tests; stage RSS',old['rss_bytes'],'->',new['rss_bytes'])
if __name__=='__main__':main()
