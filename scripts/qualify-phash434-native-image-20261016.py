"""Run whole unchanged pHash.cpp/CImg image profile with ASAN and owned fixtures only."""
from pathlib import Path
import os,subprocess,json,hashlib
R=Path(__file__).resolve().parents[1];W=R/'.dedup-work/phash434-20261016';exe=W/'native-image-profile';h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();rows=[]
env=dict(os.environ,ASAN_OPTIONS='detect_leaks=0:halt_on_error=1:abort_on_error=0:detect_stack_use_after_return=1')
for repeat in [1,2]:
 for mode in ['hamming','mh2first','mh3first','dct','text-positive','text-overflow']:
  fixture=W/'native-image-fixtures'/f'{repeat}-{mode}';fixture.mkdir(parents=True,exist_ok=False)
  p=fixture/'stdout.log';err=fixture/'stderr.log';run=subprocess.run([str(exe),mode,str(fixture)],cwd=W,capture_output=True,text=True,env=env,timeout=60);p.write_text(run.stdout);err.write_text(run.stderr)
  if mode=='text-overflow':assert run.returncode!=0 and 'AddressSanitizer: heap-buffer-overflow' in run.stderr;result={'expected_asan_heap_overflow':True}
  else:
   assert run.returncode==0,run.stdout+run.stderr;result=json.loads(run.stdout);assert not run.stderr
   if mode=='hamming':assert result['byte_pairs']==65536 and result['bit_positions']==64 and result['invalid_cases']==3
   if mode.startswith('mh'):assert result['first']==result['second'] and result['kernel_width']==(17 if mode=='mh2first' else 25)
   if mode=='dct':assert len(result['hashes'])==32 and all(v%2==0 for v in result['hashes'])
   if mode=='text-positive':assert result['matches']==1
  rows.append({'repeat':repeat,'mode':mode,'exit_code':run.returncode,'result':result,'stdout':str(p.relative_to(R)),'stdout_sha256':h(p),'stderr':str(err.relative_to(R)),'stderr_sha256':h(err),'fixtures':{str(f.relative_to(R)):h(f) for f in sorted(fixture.glob('*.ppm'))}})
for mode in ['hamming','mh2first','mh3first','dct','text-positive']:
 pair=[r for r in rows if r['mode']==mode];assert pair[0]['result']==pair[1]['result']
assert next(r for r in rows if r['mode']=='mh2first')['result']['first']!=next(r for r in rows if r['mode']=='mh3first')['result']['first']
libs=subprocess.check_output(['otool','-L',str(exe)],text=True);(W/'native-image-linked-libraries.log').write_text(libs)
libpins={}
for line in libs.splitlines()[1:]:
 path=line.strip().split(' (')[0]
 if path.startswith('/') and Path(path).is_file():libpins[path]=h(path)
p=R/'docs/research/dedup-500/phash434-native-image-controls-20261016.json';assert not p.exists();p.write_text(json.dumps({'status':'verified_selected_whole_cpp_image_profile_and_asan_boundaries','producer_sha256':h(Path(__file__)),'probe':'scripts/phash434-native-controls-20261016.cpp','probe_sha256':h(R/'scripts/phash434-native-controls-20261016.cpp'),'binary':str(exe),'binary_sha256':h(exe),'compiler':subprocess.check_output(['clang++','--version'],text=True),'build_log':str((W/'native-image-build.log').relative_to(R)),'build_log_sha256':h(W/'native-image-build.log'),'asan_options':env['ASAN_OPTIONS'],'rows':rows,'source_pins':{str(p.relative_to(R)):h(p) for p in [W/'submodule/src/pHash.cpp',W/'submodule/src/pHash.h.cmake',W/'submodule/third-party/CImg/CImg.h',W/'generated/pHash.h']},'linked_libraries_log':str((W/'native-image-linked-libraries.log').relative_to(R)),'linked_libraries_log_sha256':h(W/'native-image-linked-libraries.log'),'linked_library_pins':libpins,'scope':'Whole unchanged pHash.cpp withoriginalpinnedCImg3.95MBheader, generatedoriginalCMakeheaderconfiguration imageenabled/audio-video-disabled. Clang17gnu++14/O1/debug/ASAN; realHomebrewPNG/JPEG/TIFF libraries. No sourcebodyextraction/algorithmadapters. 12freshnativeprocesses:131072bytepairs+12864bits+6invalidHammingchecks;4MHorderprocesses realownedPNM;64actualDCTimagehashes;2positivetextmatchand2expectedASANoverflowprocesses. Python/cppyy binding notexecuted.','limits':['Selectedimageprofile/nativeAPI evidence, notoriginalCMake/fullPythonpackage/video/audio/otherbindings/platform/originalexamples/fullsource/performance qualification','OriginalCImgdependency bytespinned/compiled butnotcompletelyread; full183entrysubmodule review remainsopen','LeakSanitizerdisabled dueplatformprofile; ASANheapoverflow only, no leaktiming/hardRSS claim','ActualDCTLSB0 confirmed on64syntheticimages andshiftloop staticallyread; no generalprecision/recall oralternateDCToracle claim','LibrarysourcepublicGPL3+, no productioncodecopied/adopted']},indent=2)+'\n');print('Verified12nativeprocesses: MHfirst-parametercache,64DCTimages,2ASANoverflows; wholeimageprofile only')
