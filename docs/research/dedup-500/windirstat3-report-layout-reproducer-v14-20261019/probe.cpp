#include <string>
#include <string_view>
#include <unordered_map>
#include <sstream>
#include <iostream>
#include <cstring>
namespace Localization { std::wstring ConvertToWideString(std::string_view s){return std::wstring(s.begin(),s.end());}}
static std::string JsonUnescape(const std::string_view sv)
{
    std::string out;
    out.reserve(sv.size());
    for (size_t i = 0; i < sv.size(); ++i)
    {
        if (sv[i] == '"') break;   // closing quote
        if (sv[i] == '\\' && i + 1 < sv.size())
        {
            ++i;
            out.push_back(sv[i]);
        }
        else out.push_back(sv[i]);
    }
    return out;
}

// Parse one JSON object {…} from the stream into a key→wstring map; returns false at ']'/EOF
static bool JsonReadObject(std::istream& in, std::unordered_map<std::wstring, std::wstring>& obj)
{
    obj.clear();
    std::string linebuf;
    while (std::getline(in, linebuf))
    {
        // Trim leading whitespace
        const char* p = linebuf.c_str();
        while (*p == ' ' || *p == '\t') ++p;

        if (*p == '}') return true;   // end of this object
        if (*p == ']') return false;  // end of array
        if (*p != '"') continue;      // skip blank lines, '[', etc.

        // Parse key then locate ':'
        const char* keyStart = p + 1;
        const char* colon = strchr(keyStart, ':');
        if (!colon) continue;
        std::wstring key = Localization::ConvertToWideString(JsonUnescape(
            std::string_view(keyStart, colon)));

        // Parse value (string or bare numeric)
        const char* valp = colon + 1;
        while (*valp == ' ' || *valp == '\t') ++valp;

        std::wstring value;
        if (*valp == '"')
        {
            // Quoted string — unescape inner content
            ++valp;
            value = Localization::ConvertToWideString(JsonUnescape(
                std::string_view(valp, linebuf.data() + linebuf.size())));
        }
        else
        {
            // Bare numeric — strip trailing comma if present
            std::string_view raw(valp);
            if (raw.ends_with(',')) raw.remove_suffix(1);
            value = Localization::ConvertToWideString(raw);
        }
        obj.emplace(std::move(key), std::move(value));
    }
    return false;
}


int main(){
 std::unordered_map<std::wstring,std::wstring> obj;
 std::istringstream pretty(R"JSON({
  "Name": "a",
  "Size": 7
}
)JSON");
 if(!JsonReadObject(pretty,obj)||obj.size()!=2||obj[L"Name"]!=L"a"||obj[L"Size"]!=L"7")return 2;
 std::istringstream oneLine(R"JSON({"Name":"a","Size":7}
)JSON");
 if(JsonReadObject(oneLine,obj)||!obj.empty())return 3;
 std::istringstream packedFields(R"JSON({
  "Name":"a","Size":7
}
)JSON");
 if(!JsonReadObject(packedFields,obj)||obj.size()!=1||obj[L"Name"]!=L"a")return 4;
 std::cout<<3;
}
