"""Exercise whole unchanged locked CLI solely on owned synthetic FASTA/FASTQ fixtures."""
from pathlib import Path
import json,subprocess,hashlib,os
R=Path(__file__).resolve().parents[1];W=R/'.dedup-work/czid151-20261016';h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();exe=W/'target/debug/czid-dedup'
fasta=b'>a\nACTG\n>b\nACTG\n>c\nACGG\n'
cases=[
 ('exact_default',{'a':fasta},['-i','a','-o','o','-c','c']),
 ('prefix2_intended_collapse',{'a':fasta},['-i','a','-o','o','-l','2']),
 ('prefix0_all_collapse',{'a':fasta},['-i','a','-o','o','-l','0']),
 ('same_path_truncates',{'a':fasta},['-i','a','-o','a']),
 ('cluster_same_input_truncates',{'a':fasta},['-i','a','-o','o','-c','a']),
 ('hardlink_output_truncates',{'a':fasta},['-i','a','-o','alias']),
 ('quality_ignored_first_retained',{'a':b'@a\nACTG\n+\n!!!!\n@b\nACTG\n+\nIIII\n'},['-i','a','-o','o']),
 ('late_bad_fastq_partial_exit0',{'a':b'@a\nACTG\n+\nIIII\n@b\nACTG\n+\nI\n'},['-i','a','-o','o','-c','c']),
 ('paired_length_mismatch_partial_exit0',{'a':b'>x\nACTG\n>y\nACGG\n','b':b'>x\nAAAA\n'},['-i','a','-i','b','-o','o','-o','p']),
 ('paired_ids_mismatch_exit0',{'a':b'>x/1\nACTG\n','b':b'>x/2\nAAAA\n'},['-i','a','-i','b','-o','o','-o','p']),
 ('paired_separator_ambiguity_accepted_ascii',{'a':b'>x\nA\0\0\0\0B\n>y\nA\n','b':b'>x\nC\n>y\nB\0\0\0\0C\n'},['-i','a','-i','b','-o','o','-o','p','-c','c']),
 ('empty_input_exit0',{'a':b''},['-i','a','-o','o','-c','c']),
 ('invalid_input_exit0',{'a':b'not a sequence\n'},['-i','a','-o','o','-c','c']),
 ('missing_input_panics',{},['-i','missing','-o','o']),
 ('non_numeric_prefix_panics',{'a':fasta},['-i','a','-o','o','-l','oops']),
 ('unequal_output_count_panics',{'a':fasta},['-i','a','-o','o','-o','p']),
]
rows=[]
for repeat in [1,2]:
 for name,files,args in cases:
  fixture=W/'cli-fixtures'/f'{repeat}-{name}';fixture.mkdir(parents=True,exist_ok=False)
  for path,data in files.items():(fixture/path).write_bytes(data)
  if name=='hardlink_output_truncates':os.link(fixture/'a',fixture/'alias')
  run=subprocess.run([str(exe),*args],cwd=fixture,capture_output=True,text=True,timeout=15)
  snapshots={p.name:p.read_bytes().hex() for p in sorted(fixture.iterdir()) if p.is_file()}
  normalized=run.stderr.replace(str(fixture),'<ownedfixture>')
  row={'repeat':repeat,'case':name,'exit_code':run.returncode,'stdout':run.stdout,'stderr':normalized,'files_hex':snapshots,'fixture':str(fixture.relative_to(R))};rows.append(row)
  print(name,run.returncode,run.stdout.strip().replace('\n','; '),{k:len(bytes.fromhex(v)) for k,v in snapshots.items()})
p=R/'docs/research/dedup-500/czid151-cli-controls-20261016.json';assert not p.exists();p.write_text(json.dumps({'status':'executed_pending_independent_outcome_audit','producer_sha256':h(Path(__file__)),'binary':str(exe),'binary_sha256':h(exe),'cases':len(cases),'runs':2,'rows':rows,'scope':'Wholeunchanged lockedRustCLI actualbio0.32.0/clap2.33.3/csv1.1.3 dependencies, ownedminimalFASTA/FASTQ only. No adapters/bodyextraction/privatebio/userfiles/deletion/network. Samepath/hardlink/clusteralias cases affect onlynewownedfixtures.','limits':['Hashseparatorcounterexample usesNULASCII acceptedbyactualparser/check, notvalidbiologicalDNA; noordinaryACGTcryptographichashcollisionclaim','No realbiologicalquality/formats/platform/large-scale/mutation/concurrency/hardRSS/performance/adoption/currentroot proof']},indent=2)+'\n')
