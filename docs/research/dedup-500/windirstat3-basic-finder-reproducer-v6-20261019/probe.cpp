#include <string>
#include <string_view>
#include <vector>
#include <utility>
#include <iostream>
class Finder {public:
    static constexpr std::wstring_view s_longPath = L"\\\\?\\";
    static constexpr std::wstring_view s_longUNCPath = L"\\\\?\\UNC\\";
static std::wstring MakeLongPathCompatible(const std::wstring& path)
    {
        if (path.size() >= 3 && path[1] == L':' && path[2] == L'\\') return s_longPath.data() + path;
        if (path.starts_with(L"\\\\?")) return path;
        if (path.starts_with(L"\\\\")) return s_longUNCPath.data() + path.substr(2);
        return path;
    }

};

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
