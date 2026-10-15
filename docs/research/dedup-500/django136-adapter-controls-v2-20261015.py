"""Unchanged upstream class AST; explicit in-memory ORM adapter, NOT Django/database execution."""
from pathlib import Path
import ast,json,logging,types,hashlib
P=Path(__file__).with_name('django136-complete-source-20261015.json');d=json.loads(P.read_text());sources={x['row']['path']:x['content'] for x in d['blobs']}
for x in d['blobs']:
 b=x['content'].encode();assert len(b)==x['row']['size'];assert hashlib.sha1(b'blob '+str(len(b)).encode()+b'\0'+b).hexdigest()==x['row']['sha']
class ValidationError(Exception):pass
class GenericRelation:pass
class ModelMeta:
 def __init__(self,obj):self.related_fields=obj.relations;self.editable_fields=obj.fields;self.model_name='Obj'
class Obj:
 def __init__(self,pk,value=''):
  self._meta=types.SimpleNamespace(model=Obj);self.pk=pk;self.value=value;self.fields=[types.SimpleNamespace(name='value',empty_values=['',None])];self.relations=[];self.events=[];self.fail=False;self.deleted=False
 def save(self):
  self.events.append('save')
  if self.fail:raise RuntimeError('injected save failure')
 def delete(self):self.events.append('delete');self.deleted=True;self.pk=None
class Manager:
 def __init__(self,items=(),fail=False):self.items=list(items);self.fail=fail
 def all(self):return list(self.items)
 def remove(self,x):self.items.remove(x)
 def add(self,x):
  if self.fail:raise RuntimeError('injected add failure')
  self.items.append(x)
serializations=[]
ns=dict(ModelMeta=ModelMeta,Model=Obj,Field=object,GenericRelation=GenericRelation,ValidationError=ValidationError,List=list,Tuple=tuple,logger=logging.getLogger('disabled-adapter'),serialize=lambda *args:serializations.append(args) or 'json')
ns['logger'].setLevel(logging.CRITICAL)
node=next(x for x in ast.parse(sources['django_super_deduper/merge.py']).body if isinstance(x,ast.ClassDef));exec(compile(ast.Module(body=[node],type_ignores=[]),'unchanged-upstream-class','exec'),ns);M=ns['MergedModelInstance'];passed=[]
def check(name,fn):fn();passed.append(name);print('PASS',name)
def basic():
 p,a=Obj(1),Obj(2,'x');assert M.create(p,[a]) is p and p.value=='x' and not a.deleted
check('empty fields fill and alias kept by default',basic)
def nonempty():
 p=Obj(1,'first');M.create(p,[Obj(2,'second')]);assert p.value=='first'
check('nonempty primary value wins',nonempty)
def self_guard():
 p=Obj(1)
 try:M.create(p,[p]);assert False
 except ValueError:pass
check('self guard',self_guard)
def delete_before_failed_save():
 p,a=Obj(1),Obj(2,'x');p.fail=True
 try:M.create(p,[a],keep_old=False);assert False
 except RuntimeError:pass
 assert a.deleted and p.events==['save']
check('alias deleted before failing primary save without rollback',delete_before_failed_save)
def prior_alias_committed():
 p,a=Obj(1),Obj(2,'x')
 try:M.create(p,[a,p],keep_old=False);assert False
 except ValueError:pass
 assert a.deleted and p.value=='x' and p.events==['save']
check('later invalid alias leaves earlier mutation',prior_alias_committed)
def serialization_eager():
 before=len(serializations);M.create(Obj(1),[]);assert len(serializations)==before+2
check('serialization evaluated even with debug logger disabled',serialization_eager)
def m2m_partial():
 p,a,o=Obj(1),Obj(2),Obj(3);p.links=Manager(fail=True);a.links=Manager([o]);field=types.SimpleNamespace(get_attname=lambda:'links');m=M(p)
 try:m._handle_m2m_related_field(field,a);assert False
 except RuntimeError:pass
 assert a.links.items==[] and p.links.items==[] and m.modified_related_objects==[]
check('m2m remove precedes failed add and audit append',m2m_partial)
def o2o_partial():
 p,a,o=Obj(1),Obj(2),Obj(3);p.link=None;a.link=o;p.fail=True;p.relations=[types.SimpleNamespace(one_to_many=False,one_to_one=True,many_to_many=False,name='link')]
 try:M.create(p,[a]);assert False
 except RuntimeError:pass
 assert a.link is None and a.events==['save'] and p.link is o
check('o2o alias detached before failed primary save',o2o_partial)
def o2m_delete():
 p,a,o=Obj(1),Obj(2),Obj(3);o._meta=types.SimpleNamespace(model=Obj);o.validate_unique=lambda:(_ for _ in ()).throw(ValidationError('collision'));a.children=Manager([o]);f=types.SimpleNamespace(get_accessor_name=lambda:'children',field=types.SimpleNamespace(name='parent',null=False));m=M(p);m._handle_o2m_related_field(f,a);assert o.deleted and o.pk==3 and m.modified_related_objects==[o]
check('default nonnullable uniqueness conflict deletes child and restores audit pk',o2m_delete)
def o2m_raise():
 p,a,o=Obj(1),Obj(2),Obj(3);o._meta=types.SimpleNamespace(model=Obj);o.validate_unique=lambda:(_ for _ in ()).throw(ValidationError('collision'));a.children=Manager([o]);f=types.SimpleNamespace(get_accessor_name=lambda:'children',field=types.SimpleNamespace(name='parent',null=False));m=M(p,raise_validation_exception=True)
 try:m._handle_o2m_related_field(f,a);assert False
 except ValidationError:pass
 assert not o.deleted and not o.events and m.modified_related_objects==[] and o.parent is p
check('raise validation option prevents child deletion but leaves memory reference changed',o2m_raise)
print(json.dumps(dict(passed=len(passed),failed=0,scope='Explicit in-memory ORM adapter of unchanged class AST; no Django, actual SQL transactions, concurrency, installed package or upstream suite qualification.')))
