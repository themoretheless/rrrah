"""Original advanced-settings refresh expressions with explicit synthetic changed bindings."""
from pathlib import Path
import hashlib
import json
import re
import subprocess

R=Path(__file__).resolve().parents[1]
S=R/'.dedup-work/windirstat3-20261019/source'
W=R/'.dedup-work/windirstat3-refresh-policy-20261019'
W.mkdir(exist_ok=False)
source=S/'windirstat/Pages/PageAdvanced.cpp'
text=source.read_text(encoding='utf-8-sig')
expressions=text.split('    const bool refreshReparsepoints =',1)[1].split('    StageBinds(checkboxes);',1)[0]
expressions='const bool refreshReparsepoints ='+expressions
header=S/'windirstat/Pages/PageAdvanced.h'
binding_text=header.read_text(encoding='utf-8-sig').split('CheckboxBindings',1)[1].split('static constexpr std::array NumberBindings',1)[0]
names=re.findall(r'&COptions::(\w+)',binding_text)
assert len(names)==13
probe=W/'probe.cpp'
prefix='#include <set>\n#include <initializer_list>\n#include <iostream>\nstruct COptions { inline static bool ScanForDuplicates=false;\n'
prefix+=''.join('inline static int '+n+'=0;\n' for n in names)+'};\n'
prefix+='bool BindsChanged(const std::set<int*>& changed,std::initializer_list<int*> settings) { for(auto* p:settings)if(changed.contains(p))return true;return false;}\n'
prefix+='struct Edit {bool changed;bool Changed() const{return changed;}};\nint main(){int checks=0;int* flags[]={'
prefix+=','.join('&COptions::'+n for n in names)+'};\n'
prefix+='for(unsigned mask=0;mask<(1u<<14);mask++)for(bool active:{false,true}) {\nCOptions::ScanForDuplicates=active;std::set<int*> checkboxes;for(int i=0;i<13;i++)if(mask&(1u<<i))checkboxes.insert(flags[i]);Edit fileHashAlgorithm{bool(mask&(1u<<13))};\n'
suffix='''
bool expectedFull=(mask&((1u<<4)|(1u<<5)|(1u<<6)|(1u<<9)|(1u<<10)|(1u<<11))) || (active&&(mask&((1u<<12)|(1u<<13))));
bool expectedReparse=mask&((1u<<0)|(1u<<1)|(1u<<2)|(1u<<8));
if(refreshAll!=expectedFull||refreshReparsepoints!=expectedReparse)return 2;
checks++;
}
std::cout<<checks;
}
'''
probe.write_text(prefix+expressions+suffix)
build=subprocess.run(['clang++','-O2','-std=c++20',str(probe),'-o',str(W/'probe')],capture_output=True,text=True)
(W/'build.stderr.log').write_text(build.stderr)
assert build.returncode==0,build.stderr
run=subprocess.run([str(W/'probe')],capture_output=True,text=True,timeout=10)
assert run.returncode==0 and int(run.stdout)==32768,run.stderr
files=['windirstat/Pages/PageGeneral.cpp','windirstat/Pages/PageGeneral.h','windirstat/Pages/PageAdvanced.cpp','windirstat/Pages/PageAdvanced.h']
result=dict(status='passed_original_refresh_policy_truth_table',combinations=32768,
    whole_files_read={n:hashlib.sha256((S/n).read_bytes()).hexdigest() for n in files},changed_checkbox_order=names,
    findings=['All14change bits x duplicate-scan enabled/disabled checked: metadata/exclusion/hardlink changes request full refresh; hash/sample changes request it only with duplicate detection enabled.',
              'Reparse-following changes request reparse refresh unless full refresh takes priority.',
              'Thread-count change is staged but not part of refreshAll expression; already running worker-count change not proven.',
              'Options.h excerpt declares defaultxxHash, SampleLargeFilesfalse, scanningthreads4 bounded1..16; whole Options.h not yet reviewed.',
              'General settings stage portable-mode and shell-registration effects; none executed in this review.'],
    limits=['Original two refresh expressions compiled unchanged; BindsChanged/setting references modeled as explicit changed-bit sets.',
            'Truth-table consistency check, not actual UI/options persistence/scan execution or quality/performance proof.',
            'Four whole page files read; Options.h declarations only partial source credit.',
            'No production adoption; pHash native investigation deferred.'])
(W/'result.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({'combinations':32768,'whole_files':4}))
