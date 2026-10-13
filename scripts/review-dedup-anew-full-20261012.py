#!/usr/bin/env python3
import hashlib,json,subprocess,tempfile
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research/dedup-500';T=Path('/Users/themoretheless/.codex/tmp');S=T/'rrrah-anew-full-source-20261012';B=T/'rrrah-anew-upstream-executable-20261012';G=T/'rrrah-go-1.27.2-20261012/go/bin/go'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def digest(b):return hashlib.sha256(b).hexdigest()
def main():
 commit='d67d68db0d56469fdea7505c5a9209c0a465effa';assert subprocess.check_output(['git','-C',str(S),'rev-parse','HEAD'],text=True).strip()==commit
 paths=subprocess.check_output(['git','-C',str(S),'ls-tree','-r','--name-only','HEAD'],text=True).splitlines();assert set(paths)=={'.gitignore','LICENSE','README.mkd','go.mod','main.go','script/release'};assert not subprocess.check_output(['git','-C',str(S),'status','--porcelain'],text=True).strip();assert (S/'main.go').read_bytes()==(D/'evidence/tomnomnom--anew/02-main.go').read_bytes();testlog=T/'rrrah-anew-upstream-tests-20261012.jsonl';events=[json.loads(x)for x in testlog.read_text().splitlines()];assert not any(x.get('Test')for x in events);assert any('[no test files]'in x.get('Output','')for x in events)
 rows=[]
 def run(name,args,data,expected,before=None,after=None):
  with tempfile.TemporaryDirectory(prefix='rrrah-anew-control-')as directory:
   f=Path(directory)/'lines';
   if before is not None:f.write_bytes(before)
   command=[str(B)]+args+([str(f)]if before is not None else[]);p=subprocess.run(command,input=data,capture_output=True);assert p.returncode==0 and p.stdout==expected and p.stderr==b'',name
   if after is not None:assert f.read_bytes()==after,name
   rows.append({'control':name,'args':args,'returncode':p.returncode,'stdin_length':len(data),'stdin_sha256':digest(data),'stdout':p.stdout.decode(),'stderr':p.stderr.decode(),'before_length':len(before)if before is not None else None,'before_sha256':digest(before)if before is not None else None,'after_length':len(after)if after is not None else None,'after_sha256':digest(after)if after is not None else None})
 run('ordinary_line_membership_appends_once',[],b'Two\nThree\nThree\n',b'Three\n',b'One\nTwo\n',b'One\nTwo\nThree\n')
 run('explicit_trim_changes_line_semantics',['-t'],b' x \nx\n\tx\t\n',b'x\n')
 run('dry_run_preserves_target',['-d'],b'new\nnew\n',b'new\n',b'old\n',b'old\n')
 run('quiet_mode_appends_without_stdout',['-q'],b'new\nnew\n',b'',b'old\n',b'old\nnew\n')
 run('long_stdin_token_silently_discards_remaining_input',[],b'first\n'+b'x'*70000+b'\nlast\n',b'first\n')
 before=b'first\n'+b'x'*70000+b'\nlater\n';run('long_existing_token_misses_later_membership_and_appends_duplicate',[],b'later\n',b'later\n',before,before+b'later\n')
 manifest=D/'anew-full-source-20261012.json';out=D/'anew-full-review-20261012.json';assert not manifest.exists()and not out.exists();manifest.write_text(json.dumps({'status':'six_pinned_text_files_fully_read','repository':'tomnomnom/anew','registry_index':7,'commit':commit,'snapshot':str(S),'file_hashes':{k:sha(S/k)for k in paths},'fully_read':paths,'scope':'All6trackedtextfiles read, including fullMITlicense and release script. Release script inspected only, never executed.'},indent=2)+'\n');out.write_text(json.dumps({'status':'complete_full_source_review_six_original_cli_controls','repository':'tomnomnom/anew','registry_index':7,'upstream_go_test':'success_no_test_files_not_unit_passes','go_version':subprocess.check_output([str(G),'version'],text=True).strip(),'controls':rows,'pins':{str(p):sha(p)for p in [manifest,B,G,testlog,T/'rrrah-anew-upstream-tests-20261012.stderr',Path(__file__)]},'decision':'reject-domain_no_production_adoption','scope':'Original unchanged CLI executed in six temporary-file/stdin controls. This is line-set membership, not byte-confirmed duplicate files or image matching. Scanner token limits/errors unchecked cause silent truncation and missed membership after long existing line; complete CLI counterexamples, no concurrent mutation/race/wholeRSS/platform assertion. Source map grows with unique lines; no resource bound reproduced. All6files read for this one registry row, not full500completion.'},indent=2)+'\n');print('Full6file source review, no upstream testfiles,6originalCLI controls verified; no adoption')
if __name__=='__main__':main()
