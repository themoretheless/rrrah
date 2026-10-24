import pathlib,tempfile,json,os,subprocess,sys
from unittest.mock import patch
import discover,generate_schema
results=[]
with tempfile.TemporaryDirectory(prefix="autograph104-schema-owned-") as tmp:
 root=pathlib.Path(tmp);p=root/'unreadable.md';p.write_text('no metadata')
 with patch.object(pathlib.Path,'read_text',side_effect=OSError('owned read failure')):
  report=discover.scan_vault(root)
 assert report['meta']['total_files']==1 and report['meta']['frontmatter_coverage']==100.0
 results.append({'control':'read-error-frontmatter-coverage','observed':report['meta'],'scope':'Injected read failure, not actual permission configuration'})
 p.write_text('# Plain body\n[[target]]\n## Unique Section\n')
 report=discover.scan_vault(root)
 assert report['edge_patterns']=={} and report['section_headers']==[] and report['meta']['no_frontmatter']==1
 results.append({'control':'plain-body-patterns-skipped','edge_patterns':report['edge_patterns'],'section_headers':report['section_headers']})
command="import generate_schema,json; print(json.dumps(generate_schema.build_field_fixes({'name': {'values': {'aaab':10,'aaac':10,'aaaa':1}}}),sort_keys=True))"
variants={}
for seed in range(1,9):
 env=os.environ.copy();env['PYTHONHASHSEED']=str(seed)
 value=subprocess.check_output([sys.executable,'-c',command],cwd=pathlib.Path(__file__).parent,env=env,text=True).strip();variants.setdefault(value,[]).append(seed)
assert len(variants)>1
results.append({'control':'ambiguous-field-fix-hash-seed-dependence','variants':variants,'scope':'Same input insertion order, differing Python hash seeds; no claim these artificial names are actual synonyms'})
print(json.dumps({'passed':3,'author_controls':results,'scope':'Original discover/generate_schema functions and owned inputs; no user vault'},indent=2))
