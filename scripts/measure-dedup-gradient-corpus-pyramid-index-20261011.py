#!/usr/bin/env python3
import hashlib,json,os,shutil,subprocess,time
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp');A=T/'rrrah-gradient-corpus-pyramid-index-qualified-20261011';OUT=D/'dedup-gradient-corpus-pyramid-index-20261011.json'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
    assert not OUT.exists() and not A.exists();A.mkdir()
    corpus=T/'rrrah-dedup-copydays/prepared.json';data=json.loads(corpus.read_text());rows=sorted(data['images']['original'],key=lambda x:x['filename'])+sorted(data['images']['strong'],key=lambda x:x['filename']);assert len(rows)==386
    sources=[R/'Cargo.toml',R/'Cargo.lock',R/'crates/rrrah-dedup/Cargo.toml',*sorted((R/'crates/rrrah-dedup/src').rglob('*.rs')),R/'crates/rrrah-dedup/examples/gradient_corpus_pyramid_index_probe.rs',Path(__file__),corpus]
    live_sources={str(p):sha(p) for p in sources}
    pins={}
    for source in sources:
        if source.is_relative_to(R):
            target=A/'source'/source.relative_to(R);target.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(source,target);assert sha(target)==live_sources[str(source)];pins[str(target)]=sha(target)
        else:pins[str(source)]=sha(source)
    for row in rows:
        p=Path(row['normalized_path']);assert sha(p)==row['normalized_sha256'];pins[str(p)]=sha(p)
    binary=A/'gradient_corpus_pyramid_index_probe';source=T/'rrrah-dedup-exact-portfolio-20261011/debug/examples/gradient_corpus_pyramid_index_probe';digest=sha(source);shutil.copy2(source,binary);assert sha(binary)==digest;pins[str(binary)]=digest
    report={'status':'running','producer_pid':os.getpid(),'pins':pins,'results':[],'scope':'386 normalized 320-side corpus images, interpolated four-level maximum128 gradients per file, radius0.1; candidate union versus scalar exhaustive oracle, not image identity or original-resolution recall qualification'}
    def save():
        p=OUT.with_suffix('.tmp');p.write_text(json.dumps(report,indent=2)+'\n');p.replace(OUT)
    save()
    try:
        for name,selected in [('smoke',[rows[0],rows[157]]),('all386',rows)]:
            paths=A/(name+'.paths');paths.write_text(''.join(row['normalized_path']+'\n' for row in selected));desc=A/(name+'.descriptors');log=A/(name+'.log')
            row={'name':name,'status':'running','paths':str(paths),'paths_sha256':sha(paths)};report['results'].append(row);save();start=time.monotonic()
            with log.open('w') as out:
                proc=subprocess.Popen([str(binary),str(paths),'0.1',str(desc)],stdout=out,stderr=subprocess.STDOUT);row['pid']=proc.pid;save();code=proc.wait()
            row.update(returncode=code,elapsed_seconds=time.monotonic()-start,log=str(log),log_sha256=sha(log));save();assert code==0,log.read_text()[-2000:]
            lines=log.read_text().splitlines();features=[s.split('\t') for s in lines if s.startswith('F\t')];summary=[s.split('\t') for s in lines if s.startswith('S\t')];memory=[s.split('\t') for s in lines if s.startswith('M\t')];assert len(summary)==len(memory)==1
            s=summary[0];assert len(features)==int(s[1])==len(selected);assert [int(f[1]) for f in features]==list(range(len(selected)))
            assert sum(int(f[2]) for f in features)==int(s[2]);assert desc.stat().st_size==int(s[2])*1040
            pairs=[tuple(map(int,p.split('\t')[1:])) for p in lines if p.startswith('P\t')];assert len(pairs)==len(set(pairs))==int(s[3]);assert pairs==sorted(pairs)
            assert memory[0][1]=='0' and int(memory[0][2])<=256*1024*1024
            expected={(i,j) for i,a in enumerate(selected) for j,b in enumerate(selected) if i<j and a['group_id']==b['group_id']}
            found=set(pairs);cross={(i,j) for i,j in expected if (i<157<=j)} if name=='all386' else expected
            row.update(status='passed_oracle_equality',files=len(selected),descriptors=int(s[2]),pairs=len(pairs),hits=int(s[4]),comparisons=int(s[5]),exhaustive_comparisons=int(s[6]),index_seconds=float(s[7]),oracle_seconds=float(s[8]),managed_peak=int(memory[0][2]),descriptors_path=str(desc),descriptors_sha256=sha(desc),same_publisher_group_pairs=len(expected),same_publisher_group_proposed=len(expected&found),publisher_original_query_pairs=len(cross),publisher_original_query_proposed=len(cross&found),different_origin_proposals=len(found-expected));save()
        assert all(sha(p)==h for p,h in pins.items())
        assert all(sha(p)==h for p,h in live_sources.items())
        report['status']='complete_bounded_normalized_corpus_index_oracle';save();print(report['status'])
    except Exception as e:report.update(status='failed',error=str(e));save();raise
if __name__=='__main__':main()
