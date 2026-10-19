#include <string>
#include <string_view>
#include <iostream>
#include <cstdlib>
static std::string JsonQuote(const std::string_view utf8)
{
    std::string out;
    out.reserve(utf8.size() + 2);
    out.push_back('"');
    for (const unsigned char c : utf8)
    {
        if (c == '"') out += "\\\"";
        else if (c == '\\') out += "\\\\";
        else out.push_back(static_cast<char>(c));
    }
    out.push_back('"');
    return out;
}

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


int main(int argc,char** argv){
 if(argc!=3)return 2;std::string in;
 for(size_t i=0;argv[2][i];i+=2){char byte[]={argv[2][i],argv[2][i+1],0};in.push_back(char(strtoul(byte,nullptr,16)));}
 auto out=argv[1][0]=='q'?JsonQuote(in):JsonUnescape(in);
 std::cout.write(out.data(),out.size());
}
