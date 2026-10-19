"""Original long-path conversion under portable string scaffolding, no Windows API."""
from pathlib import Path
import hashlib
import json
import subprocess

R=Path(__file__).resolve().parents[1]
S=R/'.dedup-work/windirstat3-20261019/source'
W=R/'.dedup-work/windirstat3-long-paths-20261019'
W.mkdir(exist_ok=False)
path=S/'windirstat/Finder.h'
text=path.read_text(encoding='utf-8-sig')
method='static std::wstring MakeLongPathCompatible'+text.split('static std::wstring MakeLongPathCompatible',1)[1].split('    using REPARSE_DATA_BUFFER',1)[0]
constants='\n'.join(line for line in text.splitlines() if 'static constexpr std::wstring_view s_long' in line)
probe=W/'probe.cpp'
main=r'''
int main() {
 std::vector<std::pair<std::wstring,std::wstring>> cases={
 {LR"(C:\folder\file)",LR"(\\?\C:\folder\file)"},
 {LR"(\\server\share\file)",LR"(\\?\UNC\server\share\file)"},
 {LR"(\\?\C:\folder\file)",LR"(\\?\C:\folder\file)"},
 {LR"(\\?\UNC\server\share)",LR"(\\?\UNC\server\share)"},
 {LR"(relative\file)",LR"(relative\file)"},
 {LR"(C:relative)",LR"(C:relative)"},{L"",L""},{LR"(\rooted)",LR"(\rooted)"}};
 int count=0;
 for(auto& [input,expected]:cases) {
  auto actual=Finder::MakeLongPathCompatible(input);
  if(actual!=expected||Finder::MakeLongPathCompatible(actual)!=actual)return 2;
  count+=2;
 }
 for(size_t n=0;n<1000;n++) {
  auto suffix=std::wstring(n,L'x');
  for(auto prefix:{std::wstring(LR"(C:\данные\)"),std::wstring(LR"(\\server\share\)"),std::wstring(L"relative/")}) {
   auto transformed=Finder::MakeLongPathCompatible(prefix+suffix);
   if(Finder::MakeLongPathCompatible(transformed)!=transformed)return 3;
   if(transformed.find(suffix)==std::wstring::npos)return 4;
   count+=2;
  }
 }
 std::cout<<count;
}
'''
probe.write_text('#include <string>\n#include <string_view>\n#include <vector>\n#include <utility>\n#include <iostream>\nclass Finder {public:\n'+constants+'\n'+method+'};\n'+main)
build=subprocess.run(['clang++','-O2','-std=c++20',str(probe),'-o',str(W/'probe')],capture_output=True,text=True)
(W/'build.stderr.log').write_text(build.stderr)
assert build.returncode==0,build.stderr
run=subprocess.run([str(W/'probe')],capture_output=True,text=True,timeout=10)
assert run.returncode==0,run.stderr
names=['windirstat/Finder.h','windirstat/FinderBasic.h','windirstat/FinderBasic.cpp']
result=dict(status='passed_original_long_path_string_controls',checks=int(run.stdout),
    source_pins={n:hashlib.sha256((S/n).read_bytes()).hexdigest() for n in names},
    findings=['Original conversion preserves prefixed/relative paths; drive/UNC conversion idempotent on controls.',
              'FinderBasic uses4MiB local and64KiB remote directory buffers, falling back when file-ID enumeration unsupported.',
              'Unknown file-ID support yields index0; not a unique identity for merging files.',
              'UNC skips compressed-size fallback to avoid blocking unsupported remote IOCTLs; WSL9P capacity correction separate.'],
    limits=['Original method and constants compiled unchanged; portable standard string scaffolding.',
            'Mac wchar_t/stdlib, not Windows UTF16 filesystem resolution or real directory enumeration.',
            'No FinderBasic WinAPI/MTP/hardlink/scan/filter integration qualification.',
            'Three whole source files read; no production adoption; pHash native investigation deferred.'])
(W/'result.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({'checks':int(run.stdout)}))
