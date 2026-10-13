#!/usr/bin/env python3
import hashlib,json,re,shutil,subprocess
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp');A=T/'rrrah-complete-pair-schedule-qualified-20261011'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 assert not A.exists();A.mkdir();paths=[R/'Cargo.toml',R/'Cargo.lock',R/'crates/rrrah-dedup/Cargo.toml',R/'crates/rrrah-dedup/src/lib.rs',R/'crates/rrrah-dedup/src/pair_schedule.rs',R/'crates/rrrah-dedup/tests/pair_schedule.rs',R/'crates/rrrah-dedup/examples/complete_pair_schedule_probe.rs',Path(__file__)]
 live={str(p):sha(p) for p in paths};pins={}
 for p in paths:
  target=A/'source'/p.relative_to(R);target.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(p,target);assert sha(target)==live[str(p)];pins[str(target)]=sha(target)
 testlog=D/'dedup-complete-pair-schedule-final-source-tests-20261011.log';text=testlog.read_text();match=re.search(r'Running tests/pair_schedule.rs \(([^)]+)\)',text);assert match
 test=A/'pair_schedule';source=R/match[1];digest=sha(source);shutil.copy2(source,test);assert sha(test)==digest;pins[str(test)]=digest
 result=subprocess.run([str(test)],capture_output=True,text=True);out=A/'tests.log';out.write_text(result.stdout+result.stderr);assert result.returncode==0 and re.findall(r'test result: ok\. (\d+) passed; 0 failed;',result.stdout)==['5'];pins[str(out)]=sha(out)
 probe=A/'complete_pair_schedule_probe';source=R/'target/debug/examples/complete_pair_schedule_probe';digest=sha(source);shutil.copy2(source,probe);assert sha(probe)==digest;pins[str(probe)]=digest
 indexpath=D/'dedup-gradient-corpus-pyramid-index-20261011.json';index=json.loads(indexpath.read_text());assert index['status']=='complete_bounded_normalized_corpus_index_oracle';row=index['results'][-1];raw=Path(row['log']);assert sha(raw)==row['log_sha256'];pins[str(raw)]=sha(raw);pins[str(indexpath)]=sha(indexpath)
 output=A/'schedule.tsv';result=subprocess.run([str(probe),str(raw),'386',str(output)],capture_output=True,text=True);log=A/'probe.log';log.write_text(result.stdout+result.stderr);assert result.returncode==0,log.read_text();pins[str(log)]=sha(log);pins[str(output)]=sha(output)
 hints={tuple(map(int,l.split('\t')[1:])) for l in raw.read_text().splitlines() if l.startswith('P\t')};pairs=[tuple(map(int,l.split('\t'))) for l in output.read_text().splitlines()]
 expected={(a,b) for a in range(386) for b in range(a+1,386)};assert len(pairs)==len(expected)==74305;assert {(a,b) for a,b,h in pairs}==expected
 assert [(a,b) for a,b,h in pairs if h]==sorted(hints)
 assert all(h==1 for a,b,h in pairs[:len(hints)]) and all(h==0 for a,b,h in pairs[len(hints):]);assert {(a,b) for a,b,h in pairs if h}==hints
 corpus=T/'rrrah-dedup-copydays/prepared.json';assert sha(corpus)==index['pins'][str(corpus)];c=json.loads(corpus.read_text());files=sorted(c['images']['original'],key=lambda x:x['filename'])+sorted(c['images']['strong'],key=lambda x:x['filename']);assert len(files)==386;pins[str(corpus)]=sha(corpus)
 positives={(a,b) for a in range(157) for b in range(157,386) if files[a]['group_id']==files[b]['group_id']};assert len(positives)==229 and positives<=expected
 assert len(positives&hints)==133 and len(positives-hints)==96
 assert all(sha(p)==h for p,h in live.items());assert all(sha(p)==h for p,h in pins.items())
 report={'status':'complete_bounded_complete_pair_schedule_enumeration','tests':5,'proposal_subsets':64,'files':386,'total_pairs':74305,'proposed_pairs':len(hints),'fallback_pairs':74305-len(hints),'known_publisher_pairs_enumerated':229,'known_publisher_proposed':133,'known_publisher_fallback':96,'pins':pins,'scope':'New standalone raster-feature scheduling primitive, proposal-first and exhaustive fallback, copied tests and probe. Enumeration only: no image confirmation, no integration into the existing portfolio collection API, no complete corpus accuracy or full workspace regression claim. Callers must drain successfully and discard aggregates on cancellation.'}
 (D/'dedup-complete-pair-schedule-audit-20261011.json').write_text(json.dumps(report,indent=2)+'\n');print(report['status'],report['total_pairs'])
if __name__=='__main__':main()
