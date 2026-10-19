#include <cstdint>
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
static std::wstring FinderMtpPrefixedPath(std::wstring_view path)
{
    if (FinderMtp::IsPath(path)) return std::wstring(path);
    std::wstring prefixedPath(MtpPrefix);
    prefixedPath.append(path);
    return prefixedPath;
}

bool FinderMtp::IsPath(std::wstring_view path) noexcept
{
    return path.size() >= MtpPrefix.size() &&
        _wcsnicmp(path.data(), MtpPrefix.data(), MtpPrefix.size()) == 0;
}

std::wstring FinderMtp::StripPrefix(std::wstring_view path)
{
    return std::wstring(IsPath(path) ? path.substr(MtpPrefix.size()) : path);
}

ULONGLONG FinderMtp::RegisterPath(std::wstring path, std::wstring shellPath, PCIDLIST_ABSOLUTE shellPidl)
{
    if (path.empty() && shellPath.empty() && !shellPidl) return 0;

    // Normalize stored parsing paths so every registered MTP identity uses the same prefix.
    if (!path.empty()) path = FinderMtpPrefixedPath(path);
    if (!shellPath.empty()) shellPath = FinderMtpPrefixedPath(shellPath);

    // Allocate an unused high-bit token while holding exclusive access to the registry.
    const std::scoped_lock lock(s_pathMutex);
    ULONGLONG token = 0;
    do
    {
        token = s_nextPathToken++;
        s_nextPathToken |= PathTokenBit;
    }
    while (s_paths.contains(token));
    // Store a snapshot of every available identity under the generated token.
    s_paths.emplace(token, RegisteredPath(std::move(path), std::move(shellPath), shellPidl));
    return token;
}

void FinderMtp::UnregisterPath(ULONGLONG token) noexcept
{
    if ((token & PathTokenBit) == 0) return;
    const std::scoped_lock lock(s_pathMutex);
    s_paths.erase(token);
}

std::wstring FinderMtp::GetPath(ULONGLONG token)
{
    if ((token & PathTokenBit) == 0) return {};
    const std::scoped_lock lock(s_pathMutex);
    const auto found = s_paths.find(token);
    return found == s_paths.end() ? std::wstring() : found->second.path;
}

std::wstring FinderMtp::GetShellPath(ULONGLONG token)
{
    if ((token & PathTokenBit) == 0) return {};
    const std::scoped_lock lock(s_pathMutex);
    const auto found = s_paths.find(token);
    return found == s_paths.end() ? std::wstring() : found->second.shellPath;
}

bool FinderMtp::HasShellIdentity(ULONGLONG token)
{
    if ((token & PathTokenBit) == 0) return false;
    const std::scoped_lock lock(s_pathMutex);
    const auto found = s_paths.find(token);
    return found != s_paths.end() && (!found->second.shellPath.empty() || found->second.shellPidl);
}

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
