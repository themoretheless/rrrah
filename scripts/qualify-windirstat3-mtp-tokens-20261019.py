"""Original MTP path registry methods with synthetic PIDL storage; no COM/device calls."""
from pathlib import Path
import hashlib
import json
import subprocess

R=Path(__file__).resolve().parents[1]
S=R/'.dedup-work/windirstat3-20261019/source'
W=R/'.dedup-work/windirstat3-mtp-tokens-20261019'
W.mkdir(exist_ok=False)
source=S/'windirstat/FinderMtp.cpp';text=source.read_text(encoding='utf-8-sig')
prefix_method='static std::wstring FinderMtpPrefixedPath'+text.split('static std::wstring FinderMtpPrefixedPath',1)[1].split('static std::wstring FinderMtpManagerName',1)[0]
path_methods='bool FinderMtp::IsPath'+text.split('bool FinderMtp::IsPath',1)[1].split('std::vector<MtpDevice> FinderMtp::GetDevices',1)[0]
registry='ULONGLONG FinderMtp::RegisterPath'+text.split('ULONGLONG FinderMtp::RegisterPath',1)[1].split('PIDLIST_ABSOLUTE FinderMtp::CloneShellPidl',1)[0]
identity='bool FinderMtp::HasShellIdentity'+text.split('bool FinderMtp::HasShellIdentity',1)[1]
probe=W/'probe.cpp'
probe.write_text('''#include <cstdint>
#include <cwctype>
#include <string>
#include <string_view>
#include <mutex>
#include <thread>
#include <unordered_map>
#include <vector>
#include <set>
#include <iostream>
using ULONGLONG=uint64_t;using PCIDLIST_ABSOLUTE=void*;
int _wcsnicmp(const wchar_t* a,const wchar_t* b,size_t n){for(size_t i=0;i<n;i++){auto x=towlower(a[i]),y=towlower(b[i]);if(x!=y)return x<y?-1:1;}return 0;}
static constexpr std::wstring_view MtpPrefix=L"mtp:";
class FinderMtp {public:
 static constexpr ULONGLONG PathTokenBit=ULONGLONG{1}<<63;
 struct RegisteredPath {std::wstring path,shellPath;void* shellPidl;
  RegisteredPath(std::wstring p,std::wstring s,void* id):path(std::move(p)),shellPath(std::move(s)),shellPidl(id){} };
 inline static std::mutex s_pathMutex;
 inline static std::unordered_map<ULONGLONG,RegisteredPath> s_paths;
 inline static ULONGLONG s_nextPathToken=PathTokenBit;
 static bool IsPath(std::wstring_view) noexcept;static std::wstring StripPrefix(std::wstring_view);
 static ULONGLONG RegisterPath(std::wstring,std::wstring={},void* =nullptr);
 static void UnregisterPath(ULONGLONG) noexcept;
 static std::wstring GetPath(ULONGLONG);static std::wstring GetShellPath(ULONGLONG);
 static bool HasShellIdentity(ULONGLONG);
};
'''+prefix_method+path_methods+registry+identity+'''
int main(){
 using F=FinderMtp;
 if(F::RegisterPath(L"")!=0||!F::GetPath(1).empty()||F::HasShellIdentity(1))return 2;
 std::vector<std::vector<uint64_t>> tokens(4);std::vector<std::thread> workers;
 for(int p=0;p<4;p++)workers.emplace_back([&,p]{for(int i=0;i<5000;i++){
  auto name=std::to_wstring(p)+L"/"+std::to_wstring(i);
  auto token=F::RegisterPath(name,L"shell/"+name);
  if(F::GetPath(token)!=L"mtp:"+name||!F::HasShellIdentity(token))std::terminate();
  tokens[p].push_back(token);
 }});
 for(auto& thread:workers)thread.join();std::set<uint64_t> unique;
 for(auto& group:tokens)for(auto token:group){if(!(token&F::PathTokenBit)||!unique.insert(token).second)return 3;F::UnregisterPath(token);if(!F::GetPath(token).empty())return 4;}
 F::s_nextPathToken=F::PathTokenBit;auto occupied=F::RegisterPath(L"occupied");
 F::s_nextPathToken=UINT64_MAX-1;
 auto a=F::RegisterPath(L"a"),b=F::RegisterPath(L"b"),c=F::RegisterPath(L"c");
 if(a!=UINT64_MAX-1||b!=UINT64_MAX||c!=F::PathTokenBit+1||F::GetPath(occupied)!=L"mtp:occupied")return 5;
 if(!F::IsPath(L"MTP:test")||F::StripPrefix(L"MTP:test")!=L"test")return 6;
 std::cout<<unique.size();
}
''')
build=subprocess.run(['clang++','-O2','-std=c++20','-pthread',str(probe),'-o',str(W/'probe')],capture_output=True,text=True)
(W/'build.stderr.log').write_text(build.stderr)
assert build.returncode==0,build.stderr
run=subprocess.run([str(W/'probe')],capture_output=True,text=True,timeout=20)
assert run.returncode==0 and run.stdout.strip()=='20000',run.stderr
files=['windirstat/FinderMtp.cpp','windirstat/FinderMtp.h']
result=dict(status='passed_original_MTP_registry_methods_with_explicit_shell_identity_stub',
    registered_unique_paths=20000,producer_threads=4,wraparound_collision_control='passed',
    source_pins={n:hashlib.sha256((S/n).read_bytes()).hexdigest() for n in files},
    findings=['Original registry lock protects concurrent registration; high-bit tokens remain unique, unregister hides identity, wraparound skips occupied token.',
              'MTP enumeration stores display path separately from owned shell PIDL/parsing path; GetIndex initially0 is not normal filesystem file ID.',
              'Device correlation uses normalized alphanumeric containment and friendly-name fallback; device-match heuristic not exact file identity.',
              'Unusable shell entries skipped; failed size/time properties leave zero defaults, so metadata availability differs from local files.'],
    limits=['Original selected registry/path methods unchanged, synthetic RegisteredPath replaces PIDL clone/cleanup ownership; no COM operations.',
            'ASCII case-insensitive prefix shim and Mac strings; not Windows Unicode/portable-device enumeration or stream-read proof.',
            'No real device, PIDL clone lifetime, disconnection/cancellation/quality/performance qualification.',
            'Two whole MTP files read; no production adoption; pHash native investigation deferred.'])
(W/'result.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({'registered_unique_paths':20000,'wraparound_control':True}))
