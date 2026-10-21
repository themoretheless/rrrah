import re, subprocess
from pathlib import Path
base=Path('.dedup-work/googlephotos42-20261021/.github/workflows')
tails=[]
for name in ['store-publish.yml','bootstrap-amo.yml']:
    for i,m in enumerate(re.finditer(r'          git add state\.json\n([\s\S]*?          echo "::notice::[^\n]+)\n',(base/name).read_text())):
        tails.append((name,i,re.sub(r'\$\{\{[^}]+\}\}','v-test',m[1])))
assert len(tails)==4
cases=[('commit_failure',1,42,0,42,True,False,False),('clean',0,42,0,0,False,True,True),('changed',1,0,0,0,True,True,True),('diff_error',2,0,0,2,False,False,False),('push_failure',1,0,43,43,True,True,False)]
count=0
for name,i,tail in tails:
    for case,diff,commit,push,rc,c,p,n in cases:
        prefix=f"""set -euo pipefail
TAG=v-test
git() {{
case "$1" in
diff) echo DIFF; return {diff} ;;
-c) echo COMMIT; return {commit} ;;
push) echo PUSH; return {push} ;;
*) echo UNEXPECTED >&2; return 99 ;;
esac
}}
"""
        result=subprocess.run(['/bin/bash','--noprofile','--norc','-c',prefix+tail],capture_output=True,text=True)
        assert result.returncode==rc,(name,i,case,result)
        assert ('COMMIT' in result.stdout)==c
        assert ('PUSH' in result.stdout)==p
        assert ('::notice::' in result.stdout)==n
        count+=1;print(f'PASS {name} tail{i+1} {case} rc={rc}')
assert count==20
print('TOTAL 20 passed; original recording tails only, git replaced by inert shell function; no network or repository mutations')
