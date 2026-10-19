"""Original byte-level JSON helper bodies compared with independent Python JSON parser."""
from pathlib import Path
import hashlib
import json
import subprocess

R=Path(__file__).resolve().parents[1]
S=R/'.dedup-work/windirstat3-20261019/source'
W=R/'.dedup-work/windirstat3-json-helpers-v2-20261019'
W.mkdir(exist_ok=False)
source=S/'windirstat/CsvLoader.cpp';text=source.read_text(encoding='utf-8-sig')
quote='static std::string JsonQuote('+text.split('static std::string JsonQuote(',1)[1].split('static std::string JsonQuoteW',1)[0]
unescape='static std::string JsonUnescape('+text.split('static std::string JsonUnescape(',1)[1].split('// Parse one JSON object',1)[0]
probe=W/'probe.cpp'
probe.write_text('#include <string>\n#include <string_view>\n#include <iostream>\n#include <cstdlib>\n'+quote+unescape+'''
int main(int argc,char** argv){
 if(argc!=3)return 2;std::string in;
 for(size_t i=0;argv[2][i];i+=2){char byte[]={argv[2][i],argv[2][i+1],0};in.push_back(char(strtoul(byte,nullptr,16)));}
 auto out=argv[1][0]=='q'?JsonQuote(in):JsonUnescape(in);
 std::cout.write(out.data(),out.size());
}
''')
build=subprocess.run(['clang++','-O2','-std=c++20',str(probe),'-o',str(W/'probe')],capture_output=True,text=True)
(W/'build.stderr.log').write_text(build.stderr);assert build.returncode==0,build.stderr
def run(mode,value):
    p=subprocess.run([str(W/'probe'),mode,value.encode().hex()],capture_output=True,timeout=30)
    assert p.returncode==0
    return p.stdout.decode()
positive=['ordinary','a"b','a\\b','кириллица 😀']
for value in positive:assert json.loads(run('q',value))==value
invalid=[]
for c in range(32):
    value='a'+chr(c)+'b'
    try:json.loads(run('q',value))
    except json.JSONDecodeError:invalid.append(c)
assert invalid==list(range(32))
escape_controls=[]
for escaped in [r'\n',r'\t',r'\r',r'\b',r'\f',r'\u0041']:
    actual=run('u',escaped)
    expected=json.loads('"'+escaped+'"')
    assert actual!=expected
    escape_controls.append({'input':escaped,'actual':actual,'standard_expected':expected})
result=dict(status='original_JSON_helpers_characterized_with_independent_parser',positive_roundtrips=4,
    unescaped_control_characters_rejected_by_standard_parser=invalid,standard_escape_mismatches=escape_controls,
    source_pin={'path':'windirstat/CsvLoader.cpp','sha256':hashlib.sha256(source.read_bytes()).hexdigest()},
    findings=['JsonQuote escapes quote/backslash but leaves all32control bytes unescaped, producing invalid JSON for such strings.',
              'JsonUnescape removes slash but does not implement standard control or unicode escapes; six concrete mismatches retained.',
              'Duplicate CSV/JSON export carries sampled flag, preserving probable/fullhash distinction in reviewed source.',
              'Hardlink command/update excerpts reject sampled selections and unsettled scans; complete operation not reviewed.'],
    limits=['Original two helper bodies compiled unchanged; independent Python json parser used as oracle.',
            'Synthetic UTF8 strings, not claim normal Windows filenames legally contain control characters or actual report corpus fails.',
            'No Windows wide-to-UTF8 conversion, full report save/load, UI/hardlink operation execution or whole CsvLoader.cpp review.',
            'CsvLoader.h whole read; CsvLoader.cpp and Actions.cpp only selected excerpts; no production adoption, pHash native work deferred.'])
(W/'result.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({'positive_roundtrips':4,'unescaped_controls':32,'escape_mismatches':6}))
