"""Actual complete unchanged Cargo binary on owned small datasets and streams."""
from pathlib import Path
import json,hashlib,subprocess,re
R=Path(__file__).resolve().parents[1];W=R/'.dedup-work/text85-20261016';S=W/'source';A=W/'whole-cli-controls';A.mkdir(exist_ok=False);exe=W/'target/debug/dedup_dataset';h=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();runs=[]
for n in range(2):
 a=A/f'run{n}';a.mkdir();rows=[]
 def run(name,args,expected=0):
  p=subprocess.run([str(exe),*map(str,args)],capture_output=True,text=True,timeout=15);i=len(rows);(a/f'{i}.stdout').write_text(p.stdout);(a/f'{i}.stderr').write_text(p.stderr);assert (p.returncode==0 if expected==0 else p.returncode!=0),(name,p.returncode,p.stdout,p.stderr);rows.append({'case':name,'exit_code':p.returncode,'stdout_sha256':h(a/f'{i}.stdout'),'stderr_sha256':h(a/f'{i}.stderr')});return p
 data=a/'banana';data.write_bytes(b'banana');p=run('make_banana',['make','--data-file',data]);assert Path(str(data)+'.table.bin').read_bytes()==bytes([5,3,1,0,4,2])
 query=a/'query';query.write_bytes(b'ana')
 for label,extra in [('count_memory',[]),('count_mapped',['--load-disk'])]:
  p=run(label,['count-occurrences','--data-file',data,'--query-file',query,*extra]);assert 'Number of times present: 2' in p.stdout
 single=a/'single';single.write_bytes(b'a');run('make_single',['make','--data-file',single]);assert Path(str(single)+'.table.bin').stat().st_size==0;run('count_single_zero_width_panics',['count-occurrences','--data-file',single,'--query-file',query],1)
 empty=a/'empty';empty.write_bytes(b'');run('make_empty',['make','--data-file',empty]);run('count_empty_panics',['count-occurrences','--data-file',empty,'--query-file',query],1)
 malformed=a/'badqueries';malformed.write_bytes(b'\x01\x00\x00');run('truncated_multi_header_panics',['count-occurrences-multi','--data-file',data,'--query-file',malformed],1)
 cache=a/'cache-one';p=run('self_one_thread',['self-similar','--data-file',data,'--length-threshold','2','--cache-dir',cache,'--num-threads','1']);assert 'Duplicates found: 4' in p.stdout,p.stdout
 p=run('self_eight_threads_misses_short_data',['self-similar','--data-file',data,'--length-threshold','2','--cache-dir',a/'cache-eight','--num-threads','8']);assert 'Duplicates found: 0' in p.stdout,p.stdout
 run('zero_threads_panics',['self-similar','--data-file',data,'--length-threshold','2','--cache-dir',a/'cache-zero','--num-threads','0'],1)
 p=run('collect_one_thread_ranges',['collect','--data-file',data,'--length-threshold','2','--cache-dir',cache]);assert 'out\n1 6' in p.stdout,p.stdout
 # Different basenames avoid cache-output aliasing, same bytes expose EOF handling.
 other=a/'banana2';other.write_bytes(b'banana');Path(str(other)+'.table.bin').write_bytes(Path(str(data)+'.table.bin').read_bytes())
 p=run('across_same_data_terminal_group_panics',['across-similar','--data-file-1',data,'--data-file-2',other,'--length-threshold','2','--cache-dir',a/'cache-across','--num-threads','1'],1);assert 'Reached EOF badly' in p.stderr,p.stderr
 out=a/'merged';run('merge_one_piece',['merge','--suffix-path',data,'--output-file',out,'--num-threads','1']);assert out.read_bytes()==data.read_bytes();assert Path(str(out)+'.table.bin.0000').read_bytes()==Path(str(data)+'.table.bin').read_bytes()
 assert len(rows)==14;runs.append(rows)
assert [{k:v for k,v in x.items() if k not in ['stdout_sha256','stderr_sha256']} for x in runs[0]]==[{k:v for k,v in x.items() if k not in ['stdout_sha256','stderr_sha256']} for x in runs[1]]
build=json.loads((W/'native-build-location.json').read_text());buildsrc=Path(build['source']);original=json.loads((R/'docs/research/dedup-500/text85-fetched-source-pins-20261016.json').read_text())
for x in original['blobs']:assert h(S/x['path'])==x['sha256'] and h(buildsrc/x['path'])==x['sha256']
report={'status':'verified_complete_original_cargo_binary_14_cli_controls_twice','producer_sha256':h(Path(__file__)),'binary':str(exe.relative_to(R)),'binary_sha256':h(exe),'build_source':str(buildsrc),'build_log':{'path':str((W/'native-build.log').relative_to(R)),'sha256':h(W/'native-build.log')},'resolved_cargo_lock':{'path':str(buildsrc/'Cargo.lock'),'sha256':h(buildsrc/'Cargo.lock')},'source_pins':{x['path']:x['sha256'] for x in original['blobs']},'runs':runs,'artifact_pins':{str(p.relative_to(R)):h(p) for p in A.rglob('*') if p.is_file()},'scope':'Full unchanged originalCargo.toml/main/table andall15sourceblobs copiedbyteidentical toowned/private/tmp checkout toavoidparentworkspace. ActualCargo1.98.1 devprofileopt3/overflowchecksfalse build, resolvedunlockeddependencies recorded.14wholeCLI nativeprocesses twice, actualfilebuffer/Crossbeam/fileIO and6byteownedfixtures; no API/bodyadapters.','findings':['Onebyte make writeszero-widthtable; countpanics. Emptyfile countpanics; truncatedmultiqueryheaderpanics.','Selfsimilar banana/threshold2 finds4matches with1thread but0with8threads becauseincrement0; default8threadssmallinput false-negative nativecontrol.','Acrosssimilar identicalbanana/differentbasenames1thread panicsReachedEOFbadly interminalmatchinggroup.','Countmemory/mappedana2 andmake suffixbytes correct;1threadcollectranges1..6;onepiece mergesource/table parity correct.'],'limits':['Small ownedfixtures only; no complete dataset/paperresults/hugefile/multipartlongrepeat/HACKSIZE/platform/hardRSS/throughput/adoption proof.','Originaltree hasnoCargo.lock; resolveddependencyversions areobservedbuild, nothistoricaloriginaldependency-source qualification.','No userfiles orTensorFlow/HF/tokenizer/downloadedcorpus, no productioncodecopy.']}
p=R/'docs/research/dedup-500/text85-whole-cli-controls-20261016.json';assert not p.exists();p.write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({'status':'verified','native_processes':28}))
