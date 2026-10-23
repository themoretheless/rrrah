from pathlib import Path
from click.testing import CliRunner
import re,sys,tempfile,json,os,hashlib
root=Path(__file__).resolve().parents[3];sys.path.insert(0,str(root/".dedup-work/mail092-20261024"));source=root/".dedup-work/mail092-20261024/docs/tutorial.md"
owned=Path(tempfile.mkdtemp(prefix="mail092-tutorial-owned-",dir=root/".dedup-work"));tempfile.tempdir=str(owned)
runner=CliRunner();calls=[]
def invoke(command,args):
 result=runner.invoke(command,["--no-config",*args]);calls.append(dict(args=args,exit_code=result.exit_code));return result
ns=dict(invoke=invoke);blocks=[]
for m in re.finditer(r"^```\{click:(source|run)\}\n(.*?)^```\s*$",source.read_text(),re.M|re.S):
 code=m.group(2);code=re.sub(r"^:hide-source:\s*\n","",code);blocks.append((m.group(1),code))
assert blocks;initial=Path.cwd()
try:
 for i,(kind,code) in enumerate(blocks):exec(compile(code,f"{source}:block{i+1}","exec"),ns)
 assert Path.cwd()==initial
finally:os.chdir(initial)
assert len(calls)==5 and all(c["exit_code"]==0 for c in calls)
result=dict(scope="Original Python click:source/run tutorial blocks executed in order with CliRunner adapter adding no-config; own scratch tempdir; not Sphinx build or shell snippets",blocks_executed=len(blocks),calls=calls,source_sha256=hashlib.sha256(source.read_bytes()).hexdigest(),owned_directory=str(owned),working_directory_restored=True,limits=["Synthetic stable tutorial corpus only.","No global installation, shell profile changes or real user mailbox touched."])
(root/"docs/research/dedup-500/mail092-tutorial-result-20261024.json").write_text(json.dumps(result,indent=2)+"\n");print(json.dumps(result))
