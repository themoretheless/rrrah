"""Original line-oriented JSON reader on equivalent standard JSON layouts."""
from pathlib import Path
import hashlib
import json
import subprocess

R=Path(__file__).resolve().parents[1]
S=R/'.dedup-work/windirstat3-20261019/source'
W=R/'.dedup-work/windirstat3-json-layout-20261019'
W.mkdir(exist_ok=False)
source=S/'windirstat/CsvLoader.cpp';text=source.read_text(encoding='utf-8-sig')
methods='static std::string JsonUnescape('+text.split('static std::string JsonUnescape(',1)[1].split('enum : std::uint8_t',1)[0]
probe=W/'probe.cpp'
main=r'''
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
'''
probe.write_text('#include <string>\n#include <string_view>\n#include <unordered_map>\n#include <sstream>\n#include <iostream>\n#include <cstring>\nnamespace Localization { std::wstring ConvertToWideString(std::string_view s){return std::wstring(s.begin(),s.end());}}\n'+methods+main)
build=subprocess.run(['clang++','-O2','-std=c++20',str(probe),'-o',str(W/'probe')],capture_output=True,text=True)
(W/'build.stderr.log').write_text(build.stderr);assert build.returncode==0,build.stderr
run=subprocess.run([str(W/'probe')],capture_output=True,text=True,timeout=30)
assert run.returncode==0,run.stderr
equivalent=['{\n  "Name": "a",\n  "Size": 7\n}\n','{"Name":"a","Size":7}\n','{\n  "Name":"a","Size":7\n}\n']
assert all(json.loads(s)=={'Name':'a','Size':7} for s in equivalent)
result=dict(status='original_JSON_layout_sensitivity_characterized',cases=3,
    source_sha256=hashlib.sha256(source.read_bytes()).hexdigest(),
    findings=['Original reader accepts one-property-per-line object; equivalent one-line object yields no object.',
              'Two properties on one line retain only first; independent standard parser confirms equivalent semantics across all3fixtures.',
              'FullCsvLoader read: parent-before-child snapshot reconstruction uses saved metadata; hash flags removed and process-local MTPtokens not serialized.',
              'Localized column labels used by writer/reader; cross-language report compatibility requires separate verification.'],
    limits=['Original JsonUnescape/JsonReadObject unchanged; ASCII widening shim substitutes Windows UTF8 conversion.',
            'No complete CItem tree/load dispatcher, physical file or Windows runtime validation.',
            'Own exporter pretty layout accepted; not claim every self-exported report fails.',
            'WholeCsvLoader.cpp read across increments; no production adoption; pHash native investigation deferred.'])
(W/'result.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({'equivalent_JSON_layouts':3,'standard_parser_agrees':True}))
