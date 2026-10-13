#!/usr/bin/env python3
from pathlib import Path
import ast,json,hashlib,types
R=Path(__file__).resolve().parents[1];D=R/'docs/research/dedup-500';E=D/'evidence/mrighele--zfs-find-deduped';r=json.loads((D/'reviews/190.json').read_text());t=json.loads((E/'tree.json').read_text());assert not t['truncated'];S=Path('/Users/themoretheless/.codex/tmp/rrrah-zfs-find-deduped-source-20261014');S.mkdir(exist_ok=False);known={e['path']:E/e['local'] for e in r['available_source_evidence']+r['available_readme_license_evidence']};pins={};sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
for e in t['tree']:
 assert e['type']=='blob';b=known[e['path']].read_bytes();assert hashlib.sha1(b'blob '+str(len(b)).encode()+b'\0'+b).hexdigest()==e['sha'];(S/e['path']).write_bytes(b);pins[e['path']]=sha(S/e['path'])
assert len(pins)==3
calls=[];fixture=[];exitcode=0
class Stdout:
 def readlines(self):calls.append({'readlines_materialized_rows':len(fixture)});return list(fixture)
class Proc:
 def __init__(self,args,**kw):calls.append({'argv':args,'text':kw.get('universal_newlines',False)});self.stdout=Stdout();self.returncode=exitcode
 def __enter__(self):return self
 def __exit__(self,*args):return False
namespace={'subprocess':types.SimpleNamespace(Popen=Proc,PIPE=object()),'DEBUG':False,'INDIRECT_BLOCKS':0,'OTHER':1};tree=ast.parse((S/'zfs_find_deduped.py').read_text());functions=[n for n in tree.body if isinstance(n,ast.FunctionDef)];exec(compile(ast.Module(body=functions,type_ignores=[]),str(S/'zfs_find_deduped.py'),'exec'),namespace)
rows=[]
def check(name,fn,lines,expected=None,error=None,code=0):
 global fixture,exitcode
 fixture=lines;exitcode=code;calls.clear()
 try:
  out=fn();assert error is None;assert out==expected,(name,out,expected);err=None
 except Exception as ex:
  if error is None or not isinstance(ex,error):raise
  out=None;err=type(ex).__name__
 rows.append({'control':name,'result':out,'error':err,'child_exitcode':code,'calls':list(calls)})
f=lambda:list(namespace['find_file_indirect_blocks']('pool/fs'));g=lambda:namespace['find_dedup_blocks']('pool');normalize=lambda:[(p.decode('utf-8',errors='backslashreplace'),b) for p,b in f()]
check('file_terminated_by_blank_is_emitted',normalize,[b'path /a\n',b'Indirect blocks:\n',b'0 0 ABC\n',b'\n'],[('/a',['ABC'])])
check('last_file_without_blank_is_lost',normalize,[b'path /a\n',b'Indirect blocks:\n',b'0 0 ABC\n'],[])
check('short_indirect_record_raises',normalize,[b'path /a\n',b'Indirect blocks:\n',b'bad\n'],error=IndexError)
check('invalid_utf8_block_raises',normalize,[b'path /a\n',b'Indirect blocks:\n',b'0 0 \xff\n'],error=UnicodeDecodeError)
check('synthetic_ddt_row_parsed',g,['index 0 refcount 2 block checksumABC;\n'],{'ABC':2})
check('duplicate_ddt_key_last_wins',g,['index 0 refcount 5 block checksumABC;\n','index 1 refcount 1 block checksumABC;\n'],{'ABC':1})
check('short_ddt_record_raises',g,['index bad\n'],error=IndexError)
check('invalid_ddt_refcount_raises',g,['index 0 refcount NaN block checksumABC;\n'],error=ValueError)
check('nonzero_zdb_status_ignored_ddt',g,[],{},code=42)
check('nonzero_zdb_status_ignored_files',normalize,[],[],code=42)
check('missing_blank_before_next_path_loses_previous_file',normalize,[b'path /a\n',b'Indirect blocks:\n',b'0 0 ABC\n',b'path /b\n',b'Indirect blocks:\n',b'0 0 DEF\n',b'\n'],[('/b',['DEF'])])
check('pool_extracted_without_validation',lambda:namespace['extract_pool']('/pool/fs'),[],'')
O=D/'zfs-find-deduped-complete-review-20261014.json';assert not O.exists();O.write_text(json.dumps(dict(status='complete_three_text_file_read_twelve_original_ast_parser_controls',registry_index=190,repository=r['repository'],commit=r['commit'],source=str(S),file_hashes=pins,fully_read=list(pins),pins={str(p):sha(p) for p in [Path(__file__),E/'tree.json',D/'reviews/190.json',*[S/k for k in pins]]},controls=rows,decision='reject_domain_no_content_identity_or_code_adoption',findings=['ReportsDDTblock sharing/uniqueness, not complete file equality; shared blocks do not prove identical file bytes.','Externalzdbexitstatus ignored; empty output on failure becomes successful empty result.','Parser relies on blank line terminators; final unterminated or replaced current path omitted. Unvalidated token positions/refcounts/UTF8 can raise.','Both command outputs fully materialized withreadlines; fullDDTmap/results retained. No cancellation, memory/work cap, source generation consistency or output-format version guards.','ActualMITtext read. No upstream tests/config/dependency manifest in complete3filetree.'],scope='All3stored pinned-tree files completely read and Gitblob IDs checked.12original function AST controls with substituted Popen/stdout only; no /sbin/zdb execution, no privilege escalation, no actualZFSdump/pool/filesystem/OS qualification. Synthetic tokens exercise parser assumptions, not validated actualZDBformat or real file duplicates. Source unchanged.'),indent=2)+'\n');print(len(rows),'controls passed')
