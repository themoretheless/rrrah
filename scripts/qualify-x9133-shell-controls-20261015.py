"""Execute selected unchanged pure helpers; never source the root/FS-mutating CLI."""
from pathlib import Path
import hashlib,json,re,subprocess
R=Path(__file__).resolve().parents[1]
D=R/'docs/research/dedup-500'
source=(D/'evidence/jim-collier--x9dedupe/02-x9dedupe').read_text()
names=['fStrTrim','fStrNormalize','fstrAppend','fNormalizePath_byref']
selected=[]
for name in names:
    m=re.search(r'^function '+name+r'\([^\n]*\n.*?^}',source,re.M|re.S)
    assert m,name
    selected.append(m.group())
helpers='\n\n'.join(selected)+'\n'
cases=[
 ('ordinary_append','v=old; fstrAppend v : new; printf "%s" "$v"','old:new'),
 ('normalize_whitespace',"fStrNormalize $'  a\\t b\\n c  '",'a b c\n'),
 ('root_path_lost','v=/; fNormalizePath_byref v; printf "<%s>" "$v"','<>'),
 ('path_spaces_removed','v="/dir / file"; fNormalizePath_byref v; printf "%s" "$v"','/dir/file'),
 ('backslash_path_changed',r'''v='a\b'; fNormalizePath_byref v; printf "%s" "$v"''','a/b'),
 ('trim_leaks_previous_global','fStrTrim first >/dev/null; fStrTrim ""','first'),
 ('append_evaluates_data',r'''v='$(printf evaluated)'; fstrAppend v : tail; printf "%s" "$v"''','evaluated:tail'),
 ('path_evaluates_data',r'''v='$(printf evaluated)'; fNormalizePath_byref v; printf "%s" "$v"''','evaluated'),
 ('unquoted_output_option_split',r'''optsPass1_Useful='-o csv:/owned/log dir/out.csv'; set -- ${optsPass1_Useful}; printf '%s\n' "$#" "$@"''','3\n-o\ncsv:/owned/log\ndir/out.csv\n'),
 ('pipeline_masks_child_failure','set -e; (exit 7) | /usr/bin/tee /dev/null; printf survived','survived'),
]
# Last two controls reproduce the original expansion and pipeline operators,
# with inert values/child and /dev/null; they are not whole-CLI executions.
assert '${optsPass1_Useful}' in source
assert '${filespec_Script_rmlint_pass2} -d -x 2>&1 | tee -a "${filespec_Log}"' in source
rows=[]
for name,body,expected in cases:
    p=subprocess.run(['/bin/bash','--noprofile','--norc','-c',helpers+body],capture_output=True,text=True)
    row={'name':name,'returncode':p.returncode,'stdout':p.stdout,'stderr':p.stderr,'expected':expected}
    rows.append(row)
    assert p.returncode==0 and p.stdout==expected,row
syntax=subprocess.run(['/bin/bash','-n',str(D/'evidence/jim-collier--x9dedupe/02-x9dedupe')],capture_output=True,text=True)
assert syntax.returncode==0,syntax.stderr
print(json.dumps({'status':'passed','controls':len(rows),'source_sha256':hashlib.sha256((D/'evidence/jim-collier--x9dedupe/02-x9dedupe').read_bytes()).hexdigest(),'selected_unchanged_functions':names,'selected_body_sha256':hashlib.sha256(helpers.encode()).hexdigest(),'bash':subprocess.check_output(['/bin/bash','--version'],text=True).splitlines()[0],'rows':rows,'limits':['No whole CLI, root, rmlint, ZFS, Btrfs, XFS or generated recopy execution','Last two checks reproduce source operators with inert values, not complete fMain','Bash3 helper/syntax checks do not qualify Bash4 lowercase expansion or Linux storage tooling','No production adoption or performance measurement']},indent=2))
