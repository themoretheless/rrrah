#!/usr/bin/env python3
"""Original comparison functions only; no Python2/pysam/BAM or biological claims."""
from pathlib import Path
from array import array
import ast
import hashlib
import json

R=Path(__file__).resolve().parents[1];E=R/'docs/research/dedup-500/evidence/OpenGene--dedup'
O=R/'docs/research/dedup-opengene200-controls-20261014.json';assert not O.exists()
names={'.gitignore':'03-gitignore','LICENSE':'01-LICENSE','README.md':'00-README.md','dedup.py':'02-dedup.py'}
for item in json.loads((E/'tree.json').read_text())['tree']:
    b=(E/names[item['path']]).read_bytes();assert hashlib.sha1(b'blob '+str(len(b)).encode()+b'\0'+b).hexdigest()==item['sha']
text=(E/'02-dedup.py').read_text();part=text[text.index('def str_compare('):text.index('def dedup3(')]
namespace={};exec(compile(ast.parse(part),str(E/'02-dedup.py'),'exec'),namespace)
compare=namespace['str_compare'];block=namespace['block_compare'];checks=[]

def record(line,name,sequence,quality):return [line,name,sequence,array('B',quality)]
def pair(name,sequence,quality):return record(0,name,sequence,quality),record(1,name,'T',[30])
def equal(left,right,threshold=20):return compare(left[0],right[0],left[1],right[1],threshold)
def check(name,result):
    assert result,name;checks.append(name)

a=pair('a','AC',[30,30]);b=pair('b','AG',[30,0]);c=pair('c','AT',[30,30])
check('identical_paired_sequences_equal',equal(a,a))
check('high_quality_mismatch_refused',not equal(a,c))
check('later_low_quality_mismatch_accepted',equal(a,b))
check('low_quality_first_base_mismatch_still_refused',not equal(pair('x','AC',[0,0]),pair('y','GC',[0,0])))
check('different_total_length_refused',not equal(a,pair('long','ACC',[30,30,30])))
check('quality_weighted_similarity_is_nontransitive',equal(a,b) and equal(b,c) and not equal(a,c))
forward=[record(i,name,seq,q) for i,(name,seq,q) in enumerate([('a','AC',[30,30]),('b','AG',[30,0]),('c','AT',[30,30])])]
reverse={name:record(i+3,name,'T',[30]) for i,name in enumerate(['a','b','c'])}
saved,duplicates=block(forward,reverse)
check('already_duplicate_middle_read_eliminates_unmatched_later_read',set(saved)=={0} and set(duplicates)=={'b','c'})
check('concatenation_discards_mate_boundaries',compare(record(0,'a','AC',[30,30]),record(0,'b','ACG',[30]*3),record(1,'a','GT',[30,30]),record(1,'b','T',[30])))
check('identical_sequences_skip_quality_shape_validation',equal(pair('x','AC',[]),pair('y','AC',[])))
try:equal(pair('x','AC',[]),pair('y','AG',[]))
except IndexError:check('different_sequences_with_missing_qualities_raise',True)
else:raise AssertionError('missing quality should expose unchecked indexing')
check('orphan_forward_block_retained_without_comparison',block([record(4,'orphan','AC',[30,30])],{})==[{4:True},{}])
check('higher_threshold_can_merge_high_quality_mismatch',equal(a,c,100))
assert len(checks)==12
O.write_text(json.dumps(dict(status='twelve_original_comparison_function_controls_confirmed',repository='OpenGene/dedup',commit='f8d48bc9fd9a65103c6058be46f098545d21303a',count=len(checks),checks=checks,pins={str(E/local):hashlib.sha256((E/local).read_bytes()).hexdigest() for local in names.values()},scope='Unchanged str_compare/block_compare function source, Python3 AST and standard array quality adapter. Legacy Python2 CLI, pysam/BAM reader/writer, real sequencing data, biological correctness and clinical use not executed or qualified. No production adoption.'),indent=2)+'\n');print(f'{len(checks)} controls confirmed')
