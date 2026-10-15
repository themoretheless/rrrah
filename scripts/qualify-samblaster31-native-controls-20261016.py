"""Owned fixtures, unchanged original Makefile build and complete hash module controls."""
from pathlib import Path
import json,hashlib,subprocess
R=Path(__file__).resolve().parents[1];W=R/'.dedup-work/samblaster31-20261016';S=W/'source';A=W/'native-controls-v3';A.mkdir(exist_ok=False)
h=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
build=subprocess.run(['make','-B'],cwd=S,capture_output=True,text=True);(A/'build.log').write_text(build.stdout+build.stderr);assert build.returncode==0
code=r'''
#include "sbhash.h"
#include <cassert>
#include <cstdio>
#include <cstdlib>
#include <set>
void fatalError(const char *s){fprintf(stderr,"%s",s);exit(1);}
int main(){
 hashTable_t t;hashTableInit(&t);
 assert(hashTableInsert(&t,0));assert(!hashTableInsert(&t,0));
 for(UINT64 i=1;i<=20000;i++)assert(hashTableInsert(&t,i));
 for(UINT64 i=0;i<=20000;i++)assert(!hashTableInsert(&t,i));
 assert(t.entries==20001);puts("resize_20001_unique_and_duplicate_replay_pass");
 hashTable_t c;hashTableInit(&c,23);
 for(UINT64 i=0;i<20;i++)assert(hashTableInsert(&c,i*23));
 for(UINT64 i=0;i<20;i++)assert(!hashTableInsert(&c,i*23));
 assert(c.entries==20);puts("collision_chain20_exact_replay_pass");
 hashTable_t high;hashTableInit(&high);assert(hashTableInsert(&high,7));
 assert(!hashTableInsert(&high,7+(UINT64(1)<<63)));assert(high.entries==1);
 puts("top_bit_alias_confirmed_7_and_2pow63plus7");
 hashTable_t safe;hashTableInit(&safe);assert(hashTableInsert(&safe,7));assert(hashTableInsert(&safe,7+(UINT64(1)<<62)));assert(safe.entries==2);
 puts("bit62_distinct_pass");
 freeHashTableNodes();return 0;
}
'''
(A/'hash-controls.cpp').write_text(code)
cmd=['c++','-O3','-I',str(S),str(A/'hash-controls.cpp'),str(S/'sbhash.cpp'),'-o',str(A/'hash-controls')];b=subprocess.run(cmd,capture_output=True,text=True);(A/'hash-build.log').write_text(b.stdout+b.stderr);assert b.returncode==0
header='@HD\tVN:1.6\tSO:queryname\n@SQ\tSN:chr1\tLN:100000\n'
def line(name,flag,pos,cigar='10M',seq='AAAAAAAAAA',qual='IIIIIIIIII',tags=''):
 return f'{name}\t{flag}\tchr1\t{pos}\t60\t{cigar}\t=\t0\t0\t{seq}\t{qual}'+ ('\t'+tags if tags else '')+'\n'
def pair(name,pos=100,seq='AAAAAAAAAA',qual='IIIIIIIIII',tags=''):
 return line(name,99,pos,seq=seq,qual=qual,tags=tags)+line(name,147,pos+100,seq=seq,qual=qual,tags=tags)
def records(out):return [x.split('\t') for x in out.splitlines() if not x.startswith('@')]
def flags(out):return [int(x[1]) for x in records(out)]
base=header+pair('first')+pair('second')
cases=[
 ('mark_pair',[],base,0,[99,147,1123,1171]),
 ('remove_pair',['-r'],base,0,[99,147]),
 ('accept_marks',['-a'],base,0,[99,147,99,147]),
 ('sequence_bytes_do_not_define_identity',[],header+pair('first')+pair('second',seq='CCCCCCCCCC'),0,[99,147,1123,1171]),
 ('different_coordinates_distinct',[],header+pair('first')+pair('second',pos=101),0,[99,147,99,147]),
 ('read_groups_not_in_signature',[],header+pair('first',tags='RG:Z:a')+pair('second',tags='RG:Z:b'),0,[99,147,1123,1171]),
 ('quality_does_not_choose_survivor',[],header+pair('low',qual='!!!!!!!!!!')+pair('high'),0,[99,147,1123,1171]),
 ('singletons_reverse_normalized',[],header+line('forward',0,100)+line('reverse',16,100),0,[0,1040]),
 ('missing_header',[],pair('first'),1,None),
 ('empty_input',[],'',1,None),
 ('missing_option_value',['--maxReadLength'],'',1,None),
 ('invalid_option_value',['--maxReadLength','abc'],'',1,None),
 ('trailing_integer_junk_accepted',['--maxReadLength','500junk'],base,0,[99,147,1123,1171]),
 ('negative_maxlength_not_validated',['--maxReadLength','-1'],header,0,[]),
 ('no_final_newline_last_quality_truncated',[],(header+line('single',0,100)).rstrip('\n'),0,[0]),
 ('add_mate_tags_missing_optional_fields_crashes',['--addMateTags'],header+pair('one'),-11,None),
 ('add_mate_tags',['--addMateTags'],header+pair('one',tags='NM:i:0'),0,[99,147]),
]
runs=[]
for run in range(2):
 rows=[]
 for i,(name,args,data,rc,want) in enumerate(cases):
  p=subprocess.run([str(S/'samblaster'),'-q',*args],input=data,capture_output=True,text=True,timeout=10)
  stem=A/f'run{run}-case{i}';Path(str(stem)+'.stdin').write_text(data);Path(str(stem)+'.stdout').write_text(p.stdout);Path(str(stem)+'.stderr').write_text(p.stderr)
  assert p.returncode==rc,(name,p.returncode,p.stderr)
  if want is not None:assert flags(p.stdout)==want,(name,flags(p.stdout))
  if name=='no_final_newline_last_quality_truncated':assert records(p.stdout)[0][10]=='IIIIIIIII'
  if name=='add_mate_tags':assert all('MC:Z:10M' in x and 'MQ:i:60' in x for x in p.stdout.splitlines() if not x.startswith('@'))
  rows.append({'case':name,'exit_code':p.returncode,'flags':flags(p.stdout),'stdout_sha256':h(Path(str(stem)+'.stdout')),'stdin':str(Path(str(stem)+'.stdin').relative_to(R)),'stderr':str(Path(str(stem)+'.stderr').relative_to(R))})
 p=subprocess.run([str(A/'hash-controls')],capture_output=True,text=True,timeout=10);assert p.returncode==0; (A/f'hash-run{run}.stdout').write_text(p.stdout);runs.append(rows)
assert [{k:v for k,v in row.items() if k not in ['stdin','stderr']} for row in runs[0]]==[{k:v for k,v in row.items() if k not in ['stdin','stderr']} for row in runs[1]]
report={'status':'verified_original_make_build_17_cli_scenarios_and4_hash_controls_twice','producer_sha256':h(Path(__file__)),'source_pins':{str(p.relative_to(R)):h(p) for p in S.iterdir() if p.suffix in ['.cpp','.h'] or p.name=='Makefile'},'binary_pins':{str(p.relative_to(R)):h(p) for p in [S/'samblaster',A/'hash-controls']},'build_log':str((A/'build.log').relative_to(R)),'hash_control_source':str((A/'hash-controls.cpp').relative_to(R)),'runs':runs,'artifact_pins':{str(p.relative_to(R)):h(p) for p in A.iterdir() if p.is_file()},'scope':'Whole originalMakefile/O3 build and unchanged samblaster CLI; complete unchanged sbhash.cpp/h linked into owned assertion harness with fatalError-only adapter.17CLI scenarios+4hash controls eachtwice. No upstream test suite exists in pinned tree.','findings':['Top-bit keys alias in generic hash module; actual production signatures use bounded27bit bin offsets, so this is outside intended key domain, not proof of biological false matches.','addMateTags with no optional fields crashes in native CLI; adding existing NM tag is positive control. No final newline truncates final quality byte; integer suffix junk accepted; negative maxReadLength accepted on header-only input.','Sequence bytes/quality/RG tags deliberately absent from signature; first record survives regardless of quality. These are declared domain limitations, not file-content identity.'],'limits':['Synthetic owned SAM only, no actual sequencing dataset or comparative precision/performance/resource campaign.','No production code copied; no claim of portable arbitrary64bit key safety or thread-safe global slab reuse.']}
P=R/'docs/research/dedup-500/samblaster31-native-controls-20261016.json';assert not P.exists();P.write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({'CLI_scenarios':17,'CLI_runs':34,'hash_control_runs':8,'status':'verified'}))
