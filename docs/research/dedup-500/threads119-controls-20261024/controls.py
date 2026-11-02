import pathlib,importlib.util,tempfile,ast,json,hashlib
base=pathlib.Path(__file__).resolve().parent
source=base/'02-dedup.py';spec=importlib.util.spec_from_file_location('original_dedup',source);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
root=pathlib.Path(tempfile.mkdtemp(prefix='owned-',dir=pathlib.Path(__file__).resolve().parent))
# Execute the two exact original upstream test function bodies; bypass package imports only.
tests=ast.parse((base/'07-test_core.py').read_text());names=['test_dedup_link_reuse','test_dedup_category_burst'];env={'DedupChecker':m.DedupChecker}
for n in tests.body:
 if isinstance(n,ast.FunctionDef) and n.name in names:
  n.decorator_list=[];exec(compile(ast.Module(body=[n],type_ignores=[]),'original-tests','exec'),env);tmp=root/n.name;tmp.mkdir();env[n.name](tmp)
d=m.DedupChecker(root/'retention.json')
for i in range(101):d.record({'affiliate_link':f'link-{i}','hook_category':'old','hook_text':'old text'})
assert len(d.load())==100
assert d.check('link-0','fresh','new words')[0]
assert not d.check('link-1','fresh','new words')[0]
a=m.DedupChecker(root/'asymmetry.json');a.record({'affiliate_link':'old','hook_category':'old','hook_text':'red blue green yellow orange purple'})
assert not a.check('new','fresh','red blue')[0]
b=m.DedupChecker(root/'reverse.json');b.record({'affiliate_link':'old','hook_category':'old','hook_text':'red blue'})
assert b.check('new','fresh','red blue green yellow orange purple')[0]
c=m.DedupChecker(root/'bad.json');c.history_path.write_text('{broken')
try:c.check('new','fresh','text');raise AssertionError('must raise')
except json.JSONDecodeError:pass
print(json.dumps({'source_sha256':hashlib.sha256(source.read_bytes()).hexdigest(),'original_test_functions_passed':names,'authored_controls_passed':3,'findings':['Never-reuse-ever contract ends after100 records; oldlink allowed while retainedlink rejected','Word containment overlap is asymmetric, not symmetric similarity','Malformed history raises JSONDecodeError, not silent empty history'],'scope':'Original standalone module and2 original test bodies with package imports bypassed;3 authored controls;real owned temp files;no posting/network/browser','root':str(root)},indent=2))
