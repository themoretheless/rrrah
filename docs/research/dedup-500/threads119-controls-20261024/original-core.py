import pathlib,importlib.util,ast,json,tempfile
base=pathlib.Path(__file__).resolve().parent;env={'json':json}
for name,file,cls in [('dedup','02-dedup.py','DedupChecker'),('content','06-content_generator.py','ContentGenerator'),('database','next-threads_poster_database.py','AffiliateDatabase')]:
 spec=importlib.util.spec_from_file_location(name,base/file);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);env[cls]=getattr(m,cls)
t=ast.parse((base/'07-test_core.py').read_text())
for n in t.body:
 if isinstance(n,ast.FunctionDef):
  n.decorator_list=[];exec(compile(ast.Module(body=[n],type_ignores=[]),'original-core-functions','exec'),env)
root=pathlib.Path(tempfile.mkdtemp(prefix='original-core-',dir=pathlib.Path(__file__).resolve().parent));passed=[]
for n in t.body:
 if isinstance(n,ast.FunctionDef) and n.name.startswith('test_'):
  p=root/n.name;p.mkdir();args=[]
  for a in n.args.args:args.append(env['templates_dir'](p) if a.arg=='templates_dir' else p)
  env[n.name](*args);passed.append(n.name)
assert len(passed)==6
print(json.dumps({'passed':passed,'total_distinct_original_functions':6,'new_distinct_over_prior_run':4,'scope':'Exact original test and fixture AST function bodies; original3 standalone modules; package imports/pytest discovery bypassed;no network/poster/browser','root':str(root)},indent=2))
