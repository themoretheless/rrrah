"""Original directory-filter methods with standard regex and explicit ASCII casefold adapter."""
from pathlib import Path
import hashlib
import json
import subprocess

R=Path(__file__).resolve().parents[1]
S=R/'.dedup-work/windirstat3-20261019/source'
W=R/'.dedup-work/windirstat3-directory-filters-v2-20261019'
W.mkdir(exist_ok=False)
path=S/'windirstat/Filtering.cpp'
text=path.read_text(encoding='utf-8-sig')
anchor=text.split('std::wstring CFiltering::ExtractIncludeAnchor',1)[1].split('// In path filters',1)[0]
anchor='std::wstring CFiltering::ExtractIncludeAnchor'+anchor
methods=text.split('std::wstring_view CFiltering::WithoutTrailingBackslashes',1)[1].split('bool CFiltering::IsFilteredOut(const std::wstring& fileName',1)[0]
methods='std::wstring_view CFiltering::WithoutTrailingBackslashes'+methods
probe=W/'probe.cpp'
probe.write_text('''#include <string>
#include <string_view>
#include <vector>
#include <regex>
#include <ranges>
#include <algorithm>
#include <cwctype>
#include <iostream>
int _wcsnicmp(const wchar_t* a,const wchar_t* b,size_t n) {
 for(size_t i=0;i<n;i++) {auto x=towlower(a[i]),y=towlower(b[i]); if(x!=y)return x<y?-1:1;} return 0;
}
class CFiltering {
 public:
 inline static bool FilterActive=true;
 inline static std::vector<std::wregex> ExcludeDirsRegex,IncludeDirsRegex;
 inline static std::vector<std::wstring> IncludeDirsAnchors;
 static std::wstring ExtractIncludeAnchor(std::wstring_view,bool);
 static std::wstring_view WithoutTrailingBackslashes(std::wstring_view);
 static bool MatchesAnyPath(const std::wstring&,const std::vector<std::wregex>&);
 static bool IsFilteredOut(const std::wstring&);
};
'''+anchor+methods+r'''
int main() {
 using F=CFiltering;
 if(F::ExtractIncludeAnchor(LR"(C:\Windows\Sys*)",false)!=LR"(C:\Windows)") return 2;
 if(F::ExtractIncludeAnchor(LR"(*\foo)",false)!=L"") return 3;
 F::IncludeDirsRegex.emplace_back(LR"(C:\\keep(?:\\.*)?)",std::regex_constants::icase);
 F::IncludeDirsAnchors={LR"(C:\keep)"};
 std::vector<std::pair<std::wstring,bool>> cases={
 {LR"(C:)",false},{LR"(C:\)",false},{LR"(C:\keep)",false},{LR"(c:\KEEP\)",false},
 {LR"(C:\keep\child)",false},{LR"(C:\keep2)",true},{LR"(C:\other)",true},{LR"(D:)",true}};
 int checks=2;
 for(auto& [path,excluded]:cases) {if(F::IsFilteredOut(path)!=excluded)return 4;checks++;}
 F::ExcludeDirsRegex.emplace_back(LR"(C:\\keep\\blocked(?:\\.*)?)",std::regex_constants::icase);
 if(!F::IsFilteredOut(LR"(C:\keep\blocked)"))return 5;
 if(!F::IsFilteredOut(LR"(C:\keep\blocked\child)"))return 6;
 checks+=2;
 F::FilterActive=false;
 for(auto& [path,excluded]:cases) {if(F::IsFilteredOut(path))return 7;checks++;}
 F::FilterActive=true;F::ExcludeDirsRegex.clear();F::IncludeDirsAnchors={L""};
 if(F::IsFilteredOut(LR"(D:\anywhere)"))return 8;
 checks++;
 std::cout<<checks<<"\n";
}
''')
build=subprocess.run(['clang++','-O2','-std=c++23',str(probe),'-o',str(W/'probe')],capture_output=True,text=True)
(W/'build.stderr.log').write_text(build.stderr)
assert build.returncode==0,build.stderr
run=subprocess.run([str(W/'probe')],capture_output=True,text=True,timeout=10)
assert run.returncode==0,run.stderr+' exit='+str(run.returncode)
result=dict(status='passed_original_selected_directory_filter_methods',checks=int(run.stdout),
    source_pins={n:hashlib.sha256((S/n).read_bytes()).hexdigest() for n in ['windirstat/Filtering.cpp','windirstat/Filtering.h']},
    findings=['Include-directory anchors preserve ancestors needed to reach a selected directory; sibling prefix keep2 is excluded.',
              'Exclude directory overrides includes and applies to descendants; disabled-filter fast path bypasses patterns.',
              'Wildcard-leading include anchor is empty, so directories remain traversable until actual matches evaluated.',
              'Full source also combines size,age,name/path filters; regex compilation errors display error and omit bad pattern.'],
    limits=['Original four method bodies extracted unchanged; state and standard regex patterns supplied by harness.',
            '_wcsnicmp shim uses towlower; ASCII Windows-style path cases only, no Windows Unicode equivalence proof.',
            'No CompileFilters/GlobToRegex/options/UI/file-age runtime qualification or whole app scan.',
            'No production adoption; pHash native investigation deferred.'])
(W/'result.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({'checks':int(run.stdout)}))
