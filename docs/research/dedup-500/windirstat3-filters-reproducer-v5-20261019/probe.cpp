#include <string>
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
std::wstring CFiltering::ExtractIncludeAnchor(const std::wstring_view pattern, const bool useRegex)
{
    constexpr std::wstring_view literalEscapes = LR"(\.+*?^$|()[]{}/)";
    constexpr std::wstring_view regexSpecials = LR"(.+*?^$|()[]{})";
    constexpr std::wstring_view globWildcards = L"*?";

    std::wstring literal;
    bool truncated = false;
    for (size_t i = 0; i < pattern.size(); ++i)
    {
        const wchar_t c = pattern[i];
        if (useRegex)
        {
            if (i == 0 && c == L'^') continue;
            if (c == L'\\' && i + 1 < pattern.size() &&
                (literalEscapes.contains(pattern[i + 1]) || iswspace(pattern[i + 1]) || iswpunct(pattern[i + 1])))
            {
                literal.push_back(pattern[++i]);
                continue;
            }
            if (c == L'$' && i + 1 == pattern.size()) break;
            if (c == L'\\' || regexSpecials.contains(c))
            {
                truncated = true;
                break;
            }
        }
        else if (globWildcards.contains(c))
        {
            truncated = true;
            break;
        }
        literal.push_back(c);
    }

    if (truncated)
    {
        // Trim back to the last directory boundary; chars after the last
        // backslash are part of the wildcarded segment, not a fixed dir.
        const auto bs = literal.find_last_of(L'\\');
        literal.resize(bs == std::wstring::npos ? 0 : bs);
    }
    while (!literal.empty() && literal.back() == L'\\') literal.pop_back();
    return literal;
}

std::wstring_view CFiltering::WithoutTrailingBackslashes(std::wstring_view path)
{
    while (!path.empty() && path.back() == L'\\') path.remove_suffix(1);
    return path;
}

bool CFiltering::MatchesAnyPath(const std::wstring& path, const std::vector<std::wregex>& patterns)
{
    if (patterns.empty()) return false;

    const auto matches = [&patterns](std::wstring_view candidate)
    {
        return std::ranges::any_of(patterns,
            [&candidate](const auto& pattern) { return std::regex_match(candidate.begin(), candidate.end(), pattern); });
    };

    if (matches(path)) return true;

    const std::wstring_view trimmed = WithoutTrailingBackslashes(path);
    return trimmed != path && matches(trimmed);
}

bool CFiltering::IsFilteredOut(const std::wstring& directoryName)
{
    if (!FilterActive) return false;
    if (MatchesAnyPath(directoryName, ExcludeDirsRegex)) return true;
    if (IncludeDirsRegex.empty()) return false;
    if (MatchesAnyPath(directoryName, IncludeDirsRegex)) return false;

    // Check if path is the same as or an ancestor of any include-anchor,
    // meaning we still need to descend into this directory to reach an included one.
    return !std::ranges::any_of(IncludeDirsAnchors, [&](const auto& anchor) -> bool
    {
        if (anchor.empty()) return true;
        const std::wstring_view a = WithoutTrailingBackslashes(anchor);
        const std::wstring_view d = WithoutTrailingBackslashes(directoryName);
        if (d.empty() || d.size() > a.size()) return false;
        if (_wcsnicmp(d.data(), a.data(), d.size()) != 0) return false;
        return d.size() == a.size() || a[d.size()] == L'\\';
    });
}


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
