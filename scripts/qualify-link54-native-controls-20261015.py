"""Unchanged local modules and selected main functions on newly owned fixtures only."""
from pathlib import Path
import ast,hashlib,json,logging,os,sys,unittest,importlib.util,tempfile
R=Path(__file__).resolve().parents[1];D=R/'docs/research/dedup-500';W=R/'.dedup-work/link54-20261015';S=W/'source';sys.path.insert(0,str(S/'src'));sys.dont_write_bytecode=True
from linkarr import helpers as H
from linkarr.models import Job,Config
from linkarr.parsers.movie import MovieParser
from linkarr.parsers.tv import TVParser
logging.getLogger('linkarr').addHandler(logging.NullHandler());logging.getLogger('linkarr').propagate=False
ns=dict(os=os,Job=Job,Config=Config,logger=H.logger,clean_broken_symlinks=H.clean_broken_symlinks,find_media_files=H.find_media_files,TVParser=TVParser,MovieParser=MovieParser)
nodes=[n for n in ast.parse((S/'src/linkarr/main.py').read_text()).body if isinstance(n,ast.FunctionDef) and n.name in ['process_job','process_jobs','process_job_for_folder']];assert len(nodes)==3;exec(compile(ast.Module(body=nodes,type_ignores=[]),'original-main-functions','exec'),ns)
with tempfile.TemporaryDirectory(prefix='owned-native-',dir=W) as temporary:
 F=Path(temporary);modules=[]
 for name in ['test_base_parser','test_movie_parser','test_tv_parser']:
  spec=importlib.util.spec_from_file_location(name,S/'test'/f'{name}.py');m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);modules.append(m)
 # The original single IO test hardcodes /dummy/dest. Redirect its fixture-only
 # destination declarations; its test body and actual create_symlink stay unchanged.
 base=modules[0];owned_dest=str(F/'upstream-dest');base.TestBaseParser.setUp=lambda self:setattr(self,'dest_path',owned_dest);base.DummyParser.get_destination_path=lambda self,source_file_path:owned_dest
 suite=unittest.TestSuite(unittest.defaultTestLoader.loadTestsFromModule(m) for m in modules);count=suite.countTestCases();assert count==19;result=unittest.TextTestRunner(verbosity=2).run(suite);assert result.wasSuccessful()
 rows=[]
 def check(name,fn):fn();rows.append(name);print('PASS '+name)
 def throws(kind,fn):
  try:fn()
  except kind:return
  raise AssertionError('expected '+kind.__name__)
 src=F/'input';src.mkdir();a=src/'Movie.Title.2023.mkv';a.write_bytes(b'one');dest=F/'links';created=H.create_symlink(str(a),str(dest));assert Path(created).resolve()==a and not os.path.isabs(os.readlink(created));check('real_relative_symlink_points_to_owned_source',lambda:None)
 def broken():
  d=F/'broken';d.mkdir();os.symlink('missing',d/a.name);assert not H.symlink_exists(str(d/a.name));throws(FileExistsError,lambda:H.create_symlink(str(a),str(d)))
 check('broken_link_exists_check_misses_directory_entry_and_create_raises',broken)
 def collision():
  b=F/'other'/a.name;b.parent.mkdir();b.write_bytes(b'different');assert H.create_symlink(str(b),str(dest)) is None;assert (dest/a.name).resolve()==a
 check('different_content_same_basename_silently_keeps_existing_target',collision)
 def cleanup():
  root=F/'cleanup';sub=root/'empty';sub.mkdir(parents=True);os.symlink('missing',sub/'dead');assert H.clean_broken_symlinks(str(root))==1;assert root.exists() and not sub.exists()
 check('cleanup_removes_dead_link_and_empty_subdirectory_retains_root',cleanup)
 def live():
  d=F/'live';link=H.create_symlink(str(a),str(d));assert not H.is_broken_symlink(link);H.remove_broken_symlink(link);assert not os.path.lexists(link) and a.exists()
 check('remove_broken_symlink_also_unlinks_live_link_if_called_directly',live)
 def case():
  upper=src/'Show.S01E01.MKV';upper.write_bytes(b'x');pattern=r'.*\.(mkv|mp4|avi)$';assert str(upper) in H.find_media_files(str(src),pattern);assert not TVParser(str(F/'upper'),pattern).check_file_type_valid(str(upper))
 check('finder_case_insensitive_but_parser_case_sensitive',case)
 def disabled():
  target=F/'disabled';ns['process_jobs'](Config(jobs=[Job(str(src),str(target),'movie',enabled=False)]));assert (target/'Movie Title (2023)'/a.name).is_symlink()
 check('disabled_job_still_processed_by_actual_process_jobs',disabled)
 def firstonly():
  one=F/'first';two=F/'second';config=Config(jobs=[Job(str(src),str(one),'movie'),Job(str(src),str(two),'movie')]);ns['process_job_for_folder'](config,str(src));assert one.exists() and not two.exists()
 check('same_source_two_jobs_change_callback_only_processes_first',firstonly)
 def rootlexical():
  root=F/'lexical-root';root.mkdir();H.remove_empty_directory(str(root),str(root)+'/');assert not root.exists()
 check('empty_directory_root_guard_compares_lexical_strings',rootlexical)
 def invalidregex():throws(__import__('re').error,lambda:H.is_media_file('x.mkv','['))
 check('invalid_regex_surfaces_native_error',invalidregex)
 def broken_source():
  p=src/'Missing.Title.2024.mkv';os.symlink('absent',p);assert str(p) in H.find_media_files(str(src),r'.*\.mkv$');linked=MovieParser(str(F/'missing-dest'),r'.*\.mkv$').organize_file(str(p));assert linked is not None and H.is_broken_symlink(linked)
 check('broken_source_link_discovered_and_mirrored_as_new_broken_link',broken_source)
 def valid_existing():
  d=F/'regular';d.mkdir();file=d/a.name;file.write_bytes(b'unrelated');assert H.create_symlink(str(a),str(d)) is None and file.read_bytes()==b'unrelated'
 check('existing_regular_destination_skipped_without_content_comparison',valid_existing)
 pins={str(p.relative_to(S)):hashlib.sha256(p.read_bytes()).hexdigest() for p in S.rglob('*') if p.is_file()}
 print(json.dumps({'status':'passed','original_test_bodies':19,'upstream_fixture_adapter':'Only baseparser IO fixture destination setUp and DummyParser path getter redirected from /dummy/dest to owned tempdir.19originaltestbodies unchanged;18puretests/1realIOtest.','controls':len(rows),'cases':rows,'source_pins':pins,'python':sys.version,'scope':'Actual unchanged helpers/models/parsermodules and3selectedmainfunctions; stdlib/localFS/regex/unittest, all IO newlyownedfixtures. No mainCLI/config/jsonschema/watchdog/watcher/Docker/platform/upstreamoriginalenvironment or libraryperformance qualification.'}))
