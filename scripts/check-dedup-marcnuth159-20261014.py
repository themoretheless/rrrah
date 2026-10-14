#!/usr/bin/env python3
"""Original SimHash with real NumPy; explicit controlled spaCy fixture adapter."""
from pathlib import Path
import ast,contextlib,hashlib,importlib,io,json,re,shutil,sys,types
import numpy as np
R=Path(__file__).resolve().parents[1];D=R/'docs/research';E=D/'dedup-500/evidence/Marcnuth--deduplication';sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
row=json.loads((D/'dedup-500/reviews/159.json').read_text());tree=json.loads((E/'tree.json').read_text());assert not tree['truncated']
files={v['path']:v['local']for v in row['available_source_evidence']+row['available_readme_license_evidence']}
files.update({'.gitignore':'07-gitignore','requirements.txt':'08-requirements.txt','setup.py':'09-setup.py','tests/data/wiki_nlp.txt':'10-wiki_nlp.txt','tests/test_nlp.py':'11-test_nlp.py'})
blobs=[v for v in tree['tree']if v['type']=='blob'];assert len(blobs)==len(files)==12
F=Path('/private/tmp/rrrah-marcnuth159-controls-20261014');F.mkdir(exist_ok=False)
source=F/'source';source.mkdir();pins={}
for v in blobs:
    p=E/files[v['path']];data=p.read_bytes()
    assert hashlib.sha1(b'blob '+str(len(data)).encode()+b'\0'+data).hexdigest()==v['sha']
    target=source/v['path'];target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(data);pins[str(p)]=sha(p)
sys.path.insert(0,str(source))
try:
    importlib.import_module('deduplication.nlp')
except ModuleNotFoundError as exc:
    assert exc.name=='spacy';missing=dict(type=type(exc).__name__,module=exc.name)
else:raise AssertionError('Expected baseline missing spaCy in retained runtime')
# Only the dependency is adapted. All original package modules/functions are unchanged.
loads=[]
class Token:
    def __init__(self,text):
        self.text=text;self.norm_=text.lower();self.lemma_=text.lower()
        self.is_punct=all(not c.isalnum() for c in text);self.is_digit=text.isdigit();self.is_space=text.isspace()
        self.like_num=text.lower() in {'one','two','three'} or self.is_digit
        self.like_url=text.startswith(('http://','https://'));self.like_email='@'in text
class Pipeline:
    def __call__(self,text):
        document=Document([Token(x)for x in text.split()]);document.sents=[types.SimpleNamespace(text=x)for x in re.split(r'(?<=[.!?])\s+',text)if x];return document
    def create_pipe(self,name):return types.SimpleNamespace(name=name)
    def add_pipe(self,component,before):assert component.name=='sentencizer'and before=='parser'
class Document(list):pass
spacy=types.ModuleType('spacy')
def load(model):loads.append(model);return Pipeline()
spacy.load=load;spacy.lang=types.SimpleNamespace(en=types.SimpleNamespace(stop_words=types.SimpleNamespace(STOP_WORDS={'the','and','or','a','an'})))
sys.modules['spacy']=spacy
nlp=importlib.import_module('deduplication.nlp');sim=importlib.import_module('deduplication.simhash');long=importlib.import_module('deduplication.lsimhash')
rows=[]
def check(name,condition,details=None):
    assert condition,name
    rows.append(dict(name=name,passed=True,details=details))
def failure(name,kind,fn):
    try:fn()
    except kind as error:check(name,True,dict(error_type=type(error).__name__))
    else:raise AssertionError(name)
# Independent fixed-width signed MD5 majority oracle, no use of original helper.
def oracle(tokens):
    totals=[0]*128
    for token in tokens:
        bits=f'{int(hashlib.md5(token.encode()).hexdigest(),16):0128b}'
        for i,bit in enumerate(bits):totals[i]+=1 if bit=='1'else -1
    return ''.join('1'if x>0 else '0'for x in totals)
zeros=[];ones=[]
for i in range(100):
    token=f'token{i}';(ones if int(hashlib.md5(token.encode()).hexdigest(),16)>=2**127 else zeros).append(token)
tokens=zeros[:2]+ones[:1];original=sim._sum_hashes(tokens);expected=oracle(tokens)
check('leading_zero_md5_padding_changes_majority',original[0]=='1'and expected[0]=='0'and original!=expected,dict(tokens=tokens,original=original,oracle=expected))
check('empty_tokens_are_all_zero',sim._sum_hashes([])=='0'*128)
check('token_order_lost',sim._sum_hashes(['alpha','beta'])==sim._sum_hashes(['beta','alpha']))
check('uniform_repetition_lost',sim._sum_hashes(['alpha','beta'])==sim._sum_hashes(['alpha','beta']*17))
check('single_token_matches_full_md5_bits',sim._sum_hashes(['alpha'])==f'{int(hashlib.md5(b"alpha").hexdigest(),16):0128b}')
check('generator_matches_list',sim._sum_hashes(iter(tokens))==original)
check('unicode_is_utf8_deterministic',sim._sum_hashes(['привет'])==sim._sum_hashes(['привет']))
failure('non_string_token_attribute_error',AttributeError,lambda:sim._sum_hashes([123]))
for blocks in [1,2,4,8,16,32,64,128]:
    value=sim.simhash('alpha beta gamma',blocks)
    check(f'valid_blocks_{blocks}',len(value)==blocks and all(len(v)==128//blocks for v in value))
failure('zero_blocks_zero_division',ZeroDivisionError,lambda:sim.simhash('alpha',0))
failure('non_divisor_assertion',AssertionError,lambda:sim.simhash('alpha',3))
failure('too_many_blocks_assertion',AssertionError,lambda:sim.simhash('alpha',256))
failure('negative_block_width_value_error',ValueError,lambda:sim.simhash('alpha',-8))
check('fractional_blocks_accepted_wrong_count',len(sim.simhash('alpha',0.5))==1)
check('boolean_blocks_accepted',len(sim.simhash('alpha',True))==1)
check('empty_and_removed_only_text_collide',sim.simhash('')==sim.simhash('the and 123 https://example.org a@b.test !!!'))
check('controlled_filter_and_lemma',list(nlp.tokenize('The ALPHA 123 two https://example.org a@b.test !!!'))==['alpha'])
check('controlled_no_stopword_filter',list(nlp.tokenize('the alpha',remove_stopwords=False))==['the','alpha'])
check('controlled_no_lemmatization',list(nlp.tokenize('ALPHA',lemmatize=False))==['ALPHA'])
before=len(loads);list(nlp.tokenize('alpha'));list(nlp.tokenize('alpha'));check('language_model_reloaded_every_call',len(loads)-before==2)
common='This fixed longest sentence contains the same many harmless tokens for both documents.'
left=common+' Short red.';right=common+' Tiny blue.'
check('longest_only_ignores_changed_short_sentence',left!=right and long.lsimhash(left,n_longest=1)==long.lsimhash(right,n_longest=1))
check('zero_longest_discards_entire_document',long.lsimhash(left,n_longest=0)==sim.simhash(''))
check('negative_longest_silently_slices',long.lsimhash(left,n_longest=-1)==long.lsimhash(left,n_longest=1))
failure('readme_function_import_is_module_not_callable',TypeError,lambda:__import__('deduplication',fromlist=['simhash']).simhash('alpha'))
# Execute all four original demo tests against their unchanged public fixture,
# retaining output; these functions contain no assertions and do not qualify NLP.
original_tests=[];capture=io.StringIO()
with contextlib.redirect_stdout(capture):
    for relative in ['tests/test_nlp.py','tests/test_simhash.py']:
        p=source/relative;module=types.ModuleType(relative);module.__file__=str(p)
        exec(compile(p.read_text(),str(p),'exec'),module.__dict__)
        functions=[n for n in ast.parse(p.read_text()).body if isinstance(n,ast.FunctionDef)and n.name.startswith('test_')]
        for fn in functions:
            assert not any(isinstance(n,ast.Assert)for n in ast.walk(fn))
            getattr(module,fn.name)();original_tests.append(relative+'::'+fn.name)
(F/'original-demo-tests.log').write_text(capture.getvalue());assert len(original_tests)==4
O=D/'dedup-marcnuth159-controls-20261014.json';assert not O.exists()
O.write_text(json.dumps(dict(status='passed_bounded_original_core_controls',repository=row['repository'],commit=row['commit'],original_tree_blobs=12,source_pins=pins,producer_sha256=sha(__file__),runtime=dict(python=sys.version,numpy=np.__version__),unadapted_import=missing,adapter='Controlled whitespace Token/Pipeline spaCy fixture, no real spaCy/model execution or accuracy claim. Real NumPy/MD5/original modules unchanged. Original setup not executed.',controls=rows,original_demo_tests=original_tests,original_tests_scope='Four unchanged print-only demos executed with explicit controlled dependency adapter, original full20KiBtext fixture. No test assertions and no learned NLP qualification.',original_test_log=str(F/'original-demo-tests.log'),original_test_log_sha256=sha(F/'original-demo-tests.log'),scope='Complete12blob original source/config/license/test/text fixture review; SimHash majority, validation, token/sentence information loss and per-call model loading controls. Not text semantic accuracy, file/pixel/native identity, model packaging/install, allplatform/format/resource or all500completion. No foreign production source adopted.'),indent=2)+'\n');print(O,len(rows))
