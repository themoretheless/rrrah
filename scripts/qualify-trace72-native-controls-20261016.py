"""Import complete unchanged implementation; only owned files and local Git repositories."""
from pathlib import Path
import os,sys,json,hashlib,contextlib,io,subprocess
R=Path(__file__).resolve().parents[1];W=R/'.dedup-work/trace72-20261016';S=W/'source';A=W/'native-controls-v2';A.mkdir(exist_ok=False)
os.environ.update(GIT_CONFIG_GLOBAL='/dev/null',GIT_CONFIG_SYSTEM='/dev/null',GIT_TERMINAL_PROMPT='0')
sys.path[:0]=[str(W/'python-deps'),str(S)]
import git
from source import run_repo as m
h=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
long='result = meaningful_function(value)'
runs=[]
for n in range(2):
 base=A/f'run{n}';base.mkdir();rows=[]
 def check(name,value,expected):assert value==expected,(name,value,expected);rows.append({'case':name,'observed':value})
 def compare(a,b):
  with contextlib.redirect_stdout(io.StringIO()):return sorted(m.compare_repos(a,b))
 check('identical_qualified_line',compare({'a.py':[long]},{'b.py':[long]}),[('a.py','b.py',long)])
 check('repeated_lines_collapse',len(compare({'a.py':[long]*20},{'b.py':[long]*50})),1)
 check('file_pair_count_multiplies',len(compare({'a.py':[long],'c.py':[long]},{'b.py':[long],'d.py':[long]})),4)
 check('destination_suffix_not_filtered',compare({'a.py':[long]},{'b.txt':[long]}),[('a.py','b.txt',long)])
 check('source_suffix_filtered',compare({'a.txt':[long]},{'b.py':[long]}),[])
 check('ignore_prefix_excludes_real_identifier',compare({'a.py':['try_count = meaningful_function(value)']},{'b.py':['try_count = meaningful_function(value)']}),[])
 check('comment_literal_can_match',len(compare({'a.py':['# a sufficiently long common comment']},{'b.py':['# a sufficiently long common comment']})),1)
 check('twenty_characters_excluded',compare({'a.py':['x'*20]},{'b.py':['x'*20]}),[])
 check('twentyone_characters_included',len(compare({'a.py':['x'*21]},{'b.py':['x'*21]})),1)
 root=base/'files';root.mkdir();(root/'a.py').write_text('  Answer = "UPPERCASE literal"  \n');(root/'data.png').write_text(long);(root/'bad.py').write_bytes(b'\xff');(root/'.git').mkdir();(root/'.git/config').write_text(long);(root/'other.PNG').write_text(long);outside=base/'owned-outside.txt';outside.write_text('owned external to scan root');(root/'alias.py').symlink_to(outside.resolve())
 data=m.extract_file_contents(root)
 check('strip_and_lower_mutates_string_literal',data['a.py'],['answer = "uppercase literal"'])
 check('hidden_git_metadata_included','.git/config' in data,True)
 check('uppercase_binary_extension_included','other.PNG' in data,True)
 check('known_binary_and_bad_utf8_skipped',('data.png' in data or 'bad.py' in data),False)
 check('symlink_follows_owned_sibling',data['alias.py'],['owned external to scan root'])
 with contextlib.redirect_stdout(io.StringIO()):stats=m.calculate_statistics([('a','b',long),('a','c',long)])
 check('statistics_count_pairs_not_unique_lines',list(stats),[2,1,2])
 origin=base/'origin';repo=git.Repo.init(origin);(origin/'x.py').write_text('old');repo.index.add(['x.py']);actor=git.Actor('Owned fixture','fixture@owned.invalid');repo.index.commit('initial',author=actor,committer=actor)
 dest=base/'clone'
 with contextlib.redirect_stdout(io.StringIO()):m.clone_repository(str(origin.resolve()),dest)
 check('real_git_local_clone',(dest/'x.py').read_text(),'old')
 (origin/'x.py').write_text('new');repo.index.add(['x.py']);repo.index.commit('new',author=actor,committer=actor)
 with contextlib.redirect_stdout(io.StringIO()):m.clone_repository(str(origin.resolve()),dest)
 check('existing_clone_stays_stale',(dest/'x.py').read_text(),'old')
 poison=base/'poison';failed=False
 try:
  with contextlib.redirect_stdout(io.StringIO()):m.clone_repository(str((base/'missing-origin').resolve()),poison)
 except git.GitCommandError:failed=True
 with contextlib.redirect_stdout(io.StringIO()):m.clone_repository(str(origin.resolve()),poison)
 check('failed_clone_leaves_unvalidated_reused_directory',[failed,poison.exists(),(poison/'x.py').exists()],[True,True,False])
 logs=base/'comparison.log'
 with contextlib.redirect_stdout(io.StringIO()) as out:m.compare_repos({'a.py':[long]},{'b.py':[long]})
 logs.write_text(out.getvalue());check('custom_comparison_prints_default_repo_labels',m.SOURCE_REPO in out.getvalue() and m.DEST_REPO in out.getvalue(),True)
 runs.append(rows)
assert runs[0]==runs[1] and len(runs[0])==19
report={'status':'verified_complete_original_module_19controls_twice_real_local_git','producer_sha256':h(Path(__file__)),'source_pins':{str(p.relative_to(R)):h(p) for p in (S/'source').glob('*.py')},'dependency_versions':{'GitPython':git.__version__},'loaded_dependency_file_pins':{str(Path(v.__file__).relative_to(R)):h(Path(v.__file__)) for v in list(sys.modules.values()) if getattr(v,'__file__',None) and str(W/'python-deps') in str(v.__file__) and Path(v.__file__).is_file()},'runs':runs,'scope':'Complete unchanged run_repo/init modules, actual Python filesystem/set/statistics and GitPython3.1.46 cloning only own local Git repositories. No API/body adapters, no remote network clone, config global/system disabled in own process.19scenarios twice.','findings':['Case-insensitive literal lines and comments treated as matches; repeats collapse per filepair andmultiply across files. Destination suffix notchecked; ignored prefixes suppress real identifiers.','All UTF8 nonblacklisted file types and hidden .git files scanned; owned symlink follows sibling outside scan root.','Existing clonedirectory notvalidated/refreshed; failed clone leaves directory then futurecalls silently reuse it. Custom comparisons print hardcoded repositorylabels.'],'limits':['No upstream tests in pinned tree; no real remoteGit repositories, history/provenance/attribution, dependency-source/security, performance or production adoption qualification.','Line intersection is not file-byte/image identity or semantic code-copy evidence. Case/whitespace normalization deliberately part of algorithm.']}
p=R/'docs/research/dedup-500/trace72-native-controls-20261016.json';assert not p.exists();p.write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({'scenarios':19,'runs':38,'status':'verified'}))
