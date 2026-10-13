#!/usr/bin/env python3
"""Copied frozen raster tests and isolated alternating exact index build measurement."""
import hashlib,json,re,shutil,statistics,subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];D=ROOT/'docs/research'
OUT=D/'dedup-gradient-index-row-major-formatted-20261011.json';ARCHIVE=Path('/Users/themoretheless/.codex/tmp/rrrah-gradient-index-row-major-formatted-qualified-20261011')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 assert not OUT.exists() and not ARCHIVE.exists();ARCHIVE.mkdir()
 mp=D/'dedup-gradient-index-row-major-formatted-snapshot-20261011.json';m=json.loads(mp.read_text());source=Path(m['snapshot'])
 assert all(sha(source/p)==h for p,h in m['file_hashes'].items())
 bp=D/'dedup-gradient-index-row-major-formatted-build-artifacts-20261011.jsonl';rows=[json.loads(l) for l in bp.read_text().splitlines()]
 assert rows[-1]=={'reason':'build-finished','success':True}
 artifacts={r['target']['name']:r for r in rows if r.get('reason')=='compiler-artifact' and r.get('executable')}
 assert set(artifacts)=={'rrrah_dedup','gradient_index','gradient_index_reflection'}
 report={'status':'running','source_manifest':str(mp),'results':[],'measurements':[]}
 def save():OUT.write_text(json.dumps(report,indent=2)+'\n')
 save();copies={}
 for name,r in artifacts.items():
  assert str(source) in r['package_id'] and r['profile']['opt_level']=='3';original=Path(r['executable']);copied=ARCHIVE/name;shutil.copy2(original,copied);assert sha(original)==sha(copied);copies[name]=copied
 for name,binary in copies.items():
  listing=subprocess.check_output([str(binary),'--list'],text=True);ignored=subprocess.check_output([str(binary),'--list','--ignored'],text=True)
  names=set(re.findall(r'^(.+): test$',listing,re.M));skipped=set(re.findall(r'^(.+): test$',ignored,re.M));expected=names-skipped
  p=subprocess.run([str(binary),'--test-threads=1'],capture_output=True,text=True);log=ARCHIVE/(name+'.log');log.write_text(p.stdout+p.stderr)
  assert p.returncode==0 and set(re.findall(r'^test (\S+) \.\.\. ok$',p.stdout,re.M))==expected
  assert re.findall(r'test result: ok\. (\d+) passed; 0 failed;',p.stdout)==[str(len(expected))]
  report['results'].append({'suite':name,'tests':len(expected),'ignored':len(skipped),'binary':str(binary),'binary_sha256':sha(binary),'log':str(log),'log_sha256':sha(log)});save()
 assert {r['suite']:r['tests'] for r in report['results']}['gradient_index']==7
 assert {r['suite']:r['tests'] for r in report['results']}['gradient_index_reflection']==2
 p=subprocess.run([str(copies['rrrah_dedup']),'gradient_index::row_major_tests::row_major_index_build_latency_probe','--ignored','--nocapture','--test-threads=1'],capture_output=True,text=True)
 log=ARCHIVE/'benchmark.log';log.write_text(p.stdout+p.stderr);assert p.returncode==0 and '1 passed; 0 failed' in p.stdout
 records=re.findall(r'ROW_MAJOR_INDEX_BENCH control=(\w+) baseline=(true|false) round=(\d+) elapsed_ns=(\d+) peak=(\d+) axis=(\d+)',p.stdout);assert len(records)==160
 measurements=[{'control':name,'baseline':base=='true','round':int(round),'elapsed_ns':int(ns),'peak':int(peak),'axis':int(axis)} for name,base,round,ns,peak,axis in records]
 summary={}
 for name in ['dense512','dense8192','dense32768','flat8192','sparse8192']:
  group=[r for r in measurements if r['control']==name];a=[r['elapsed_ns'] for r in group if r['baseline']];b=[r['elapsed_ns'] for r in group if not r['baseline']]
  assert len(a)==len(b)==16 and len({r['peak'] for r in group})==1 and len({r['axis'] for r in group})==1
  summary[name]={'baseline_median_ns':statistics.median(a),'candidate_median_ns':statistics.median(b),'baseline_over_candidate':statistics.median(a)/statistics.median(b)}
 report.update(status='complete_bounded_frozen_row_major_index_controls',tests=sum(r['tests'] for r in report['results']),benchmark_invocations=1,benchmark_builds=160,measurements=measurements,summary=summary,benchmark_log=str(log),benchmark_log_sha256=sha(log),pins={str(p):sha(p) for p in [mp,bp,Path(__file__),D/'dedup-gradient-index-row-major-formatted-build-20261011.log',Path(m['producer']),Path(json.loads(Path(m['parent_manifest']).read_text())['test_helper'])]},scope='Synthetic exact descriptor index construction, same-process alternating old reference/new implementation, opt-level3 with atomic caller checkpoints. Not whole-file or collection throughput. Production root remains unmodified.');save();print(json.dumps({'tests':report['tests'],'summary':summary}))
if __name__=='__main__':main()
