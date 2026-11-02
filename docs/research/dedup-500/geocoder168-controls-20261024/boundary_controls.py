"""Original slugify method and geocode handler, with real Flask3.1.2.

Only missing/empty address branches run; no SQL, dedupe, network or model reads.
This does not qualify the upstream pinned Flask0.10.1/Python3.4 runtime.
"""
import ast
import hashlib
import importlib.metadata
import json
from pathlib import Path
import re
from flask import Blueprint, Flask, request, make_response

directory = Path(__file__).resolve().parents[1] / 'evidence/dedupeio--dedupe-geocoder/remaining-pinned'
etl_source = directory / 'geocoder/data_loader.py'
tree = ast.parse(etl_source.read_text())
cls = next(n for n in tree.body if isinstance(n,ast.ClassDef) and n.name=='ETLThing')
slugify = next(n for n in cls.body if isinstance(n,ast.FunctionDef) and n.name=='slugify')
scope = {'re':re}
exec(compile(ast.Module(body=[slugify],type_ignores=[]),str(etl_source),'exec'),scope)
slug = lambda text: scope['slugify'](None,text)
assert slug('House-No')==slug('House No')=='house_no'
assert slug('house\nnumber')=='house\nnumber'
api_source = directory / 'geocoder/api.py'
tree = ast.parse(api_source.read_text())
handler = next(n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name=='geocode')
blueprint = Blueprint('api',__name__)
scope={'api':blueprint,'request':request,'make_response':make_response,'json':json,'dthandler':lambda obj:None}
exec(compile(ast.Module(body=[handler],type_ignores=[]),str(api_source),'exec'),scope)
app = Flask(__name__)
app.register_blueprint(blueprint)
observations=[]
with app.test_client() as client:
    for path in ['/geocode/','/geocode/?address=']:
        response=client.get(path)
        assert response.status_code==200 and response.get_json()['status']=='error'
        observations.append({'request':path,'actual_status':response.status_code,'body':response.get_json()})
result={'scope':__doc__,'runtime_flask':importlib.metadata.version('Flask'),'source_sha256':{str(p.relative_to(directory)):hashlib.sha256(p.read_bytes()).hexdigest() for p in [etl_source,api_source]},'authored_controls_run':4,'slug_controls':[{'inputs':['House-No','House No'],'output':'house_no'},{'input':'house newline number','newline_preserved':True}],'http_controls':observations,'original_tests_run':0}
Path(__file__).with_name('boundary-results.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result,indent=2))
