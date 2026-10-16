"""Audit native comparator semantics and current Rust complete-sequence regression gate."""
from pathlib import Path
import json,hashlib,re,subprocess,tomllib
R=Path(__file__).resolve().parents[1];D=R/'docs/research';W=R/'.dedup-work/phash434-20261016';h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
foreign=D/'dedup-500/phash434-native-video-distance-20261016.json';v=json.loads(foreign.read_text());assert v['producer_sha256']==h(R/'scripts/qualify-phash434-video-distance-20261016.py') and h(v['binary'])==v['binary_sha256'] and h(R/v['probe'])==v['probe_sha256']
for key in ['source_pins','logs']:
 for p,sha in v[key].items():assert h(R/p)==sha,p
assert len(v['rows'])==6
for row in v['rows']:
 for k in ['stdout','stderr']:assert h(R/row[k])==row[k+'_sha256']
 if row['mode']=='stack-limit':assert row['returncode']!=0 and 'AddressSanitizer: stack-overflow' in (R/row['stderr']).read_text() and row['result']['child_stack_limit_bytes']==8388608
 else:
  assert row['returncode']==0 and not (R/row['stderr']).read_text() and json.loads((R/row['stdout']).read_text())==row['result']
  if row['mode']=='oracle':assert row['result']['sequences']==120 and row['result']['comparisons']==43200
  if row['mode']=='boundaries':assert row['result']=={'mode':'boundaries','subset_score':1,'reverse_subset_score':1,'threshold64_score':1,'negative_threshold_score':0,'empty_is_nan':True,'one_empty_is_nan':True}
for filename in ['src/cimgffmpeg.h','src/cimgffmpeg.cpp']:
 entry=next(e for e in json.loads((W/'submodule-tree.json').read_text())['tree'] if e['path']==filename);b=(W/'submodule'/filename).read_bytes();assert hashlib.sha1(b'blob '+str(len(b)).encode()+b'\0'+b).hexdigest()==entry['sha']
assert not any(name in (W/'native-video-undefined-symbols.log').read_text() for name in ['GetNumberVideoFrames','NextFrames','ReadFrames','vfinfo_close','_fps'])
current=D/'dedup-current-pixel-sequence-v3-20261016.json';c=json.loads(current.read_text());assert c['producer_sha256']==h(R/'scripts/qualify-dedup-current-pixel-sequence-v3-20261016.py') and h(c['binary'])==c['binary_sha256'];CW=Path(c['harness']).parent;assert h(c['harness'])==c['harness_sha256']
for group in ['source_pins','logs']:
 for p,sha in c[group].items():assert h(R/p)==sha
assert h(R/c['focused_lock'])==c['focused_lock_sha256'] and tomllib.loads((R/c['focused_lock']).read_text())['package']==c['focused_lock_packages']
source_count=0
for module in ['pixels','linear','sequence']:
 for kind,prefix in [('src',''),('tests','test_')]:
  p=R/f'crates/rrrah-dedup/{kind}/{module}.rs';assert p.read_bytes()==(CW/(prefix+module+'.rs')).read_bytes();source_count+=p.read_text().count('#[test]')
s=(R/'crates/rrrah-dedup/src/local.rs').read_text();error=s[s.index('#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]'):s.index('\n/// Extract deterministic')];helper=s[s.index('pub(crate) fn reserve_slot'):s.index('\n#[allow(clippy::too_many_lines)]')];expected='extern crate self as rrrah_dedup;\nmod local {\n'+error+'\n'+helper+'\n}\n'+''.join(f'#[path="{m}.rs"]pub mod {m};\n#[path="test_{m}.rs"]mod {m}_controls;\n' for m in ['pixels','linear','sequence']);assert expected==Path(c['harness']).read_text()
names=re.findall(r'^(.*): test$',subprocess.check_output([c['binary'],'--list'],text=True),re.M);assert names==c['inventory'] and len(names)==source_count==22
prior=json.loads((D/'dedup-current-pixel-sequence-20261016.json').read_text());assert set(prior['inventory']).issubset(names) and len(set(names)-set(prior['inventory']))==1
assert 'sequence_controls::ordered_subsets_do_not_establish_complete_sequence_or_timeline_identity' in names
for i in [1,2]:
 text=(CW/f'tests-{i}.log').read_text();assert '22 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out' in text and sorted(re.findall(r'^test (\S+) \.\.\. ok$',text,re.M))==sorted(names)
old=D/'dedup-500/phash434-wrapper-and-native-cpp-audit-20261016.json';o=json.loads(old.read_text());assert o['whole_repo_complete']==False
out=D/'dedup-500/phash434-video-and-root-sequence-audit-20261016.json';assert not out.exists();out.write_text(json.dumps({'status':'verified_selected_native_video_algorithm_and_current_root_sequence_boundaries','repository':'dries007/pHashPy','commit':'3fefabf71a64bed151e44d2d045cd27f68a5d698','whole_repo_complete':False,'whole_implementation_complete':False,'auditor_sha256':h(Path(__file__)),'previous_cpp_audit':str(old.relative_to(R)),'previous_cpp_audit_sha256':h(old),'native_video_proof':str(foreign.relative_to(R)),'native_video_proof_sha256':h(foreign),'native_comparisons':86400,'expected_asan_stack_overflows':2,'current_root_proof':str(current.relative_to(R)),'current_root_proof_sha256':h(current),'current_root_tests_per_run':22,'current_root_runs':2,'source_review_scope':'All15wrapperfiles previousstage; additionalcompletecimgffmpeg.h/cpp read/pinned, cumulative6nativecore/header/buildfiles read. Other183entrysubmodule/CImg/media/bindings remainspartial. OriginalvideoIO syntaxcheck failsinstalledFFmpegremovedAPIs; comparatorcompiledfromwholeoriginalpHash.cpp withdeadstripunusedIO,no stubs/extractedbodies.','findings':v['findings'],'current_root_conclusion':'NewpublicAPIregression coversall30propernonemptyorderedsubsets of5frames atPages/Animation, bothdirections/same_sequence+same_timeline,240negativecomparisons.4identitypositivesand2emptyrefusals retainintendedcontracts. Allprior21tests retained plusnewtest:22twice44passes. RealfocusedCargo/BLAKE3/thiserror withrootconstraints, separatelyresolvedlock; no originalrrrah-dedup/default+decode/fullcrate/platform gate claim.','decision':'LCSsubsetscoreisoverlap evidence only, notcompletevideoidentity. Ourcomplete-sequence/timelinecoverage behavior confirmedandlockedbyregression. Keepresource/empty/erroradmission separate; no foreigncodeadopted. Continue fullsubmodule andcurrentlibraryquality/decoder/platform/resource coverage.','limits':v['limits']+c['limits']},indent=2)+'\n');print('Verified86400nativecomparisons/2expectedASANfailures andcurrent22tests twice44passes;fullrepositoryfalse')
