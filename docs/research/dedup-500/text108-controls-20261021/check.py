import pathlib,subprocess,json,hashlib
p=pathlib.Path('.dedup-work/text108-20261021/dedup_cpp/dedup.cpp');s=p.read_text();signature='std::string clean_utf8(std::string_view input) {';i=s.index(signature);a=s.index('{',i);j=a+1;depth=1
while depth:
 depth+=(s[j]=='{')-(s[j]=='}');j+=1
body=s[i:j];out=pathlib.Path('.dedup-work/text108-controls-20261021');out.mkdir(exist_ok=True)
code='#include <string>\n#include <string_view>\n#include <cstdio>\n'+body+r'''
int main(int argc,char**argv){std::string input;for(int i=0;argv[1][i];i+=2){unsigned int v;sscanf(argv[1]+i,"%2x",&v);input.push_back((char)v);}auto result=clean_utf8(input);for(unsigned char c:result)printf("%02x",c);return 0;}
'''
(out/'control.cpp').write_text(code);r=subprocess.run(['c++','-std=c++17',str(out/'control.cpp'),'-o',str(out/'control')],capture_output=True,text=True);assert r.returncode==0,r.stderr
cases=[]
for b in ['Привет🙂'.encode(),b'ASCII',bytes.fromhex('c0af'),bytes.fromhex('eda080'),bytes.fromhex('f4908080'),bytes.fromhex('f5808080'),b'\xffA',b'A']:
 result=subprocess.run([str(out/'control'),b.hex()],capture_output=True,text=True,check=True).stdout;cleaned=bytes.fromhex(result)
 try:b.decode('utf-8');valid=True
 except UnicodeDecodeError:valid=False
 cases.append({'input_hex':b.hex(),'strict_utf8_valid':valid,'output_hex':result,'unchanged':b==cleaned})
assert sum(not x['strict_utf8_valid'] and x['unchanged'] for x in cases)==4
assert cases[-2]['output_hex']==cases[-1]['output_hex']=='41'
r={'source_sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'method':'Original clean_utf8 function brace-extracted unchanged, compiled AppleC++17. No pybind11, Abseil, Faiss, xxhash, OpenMP, pHash or ASAN execution. Python strict UTF8 decoder used as independent validity oracle.','cases':cases,'invalid_sequences_retained_unchanged':4,'distinct_input_bytes_collapse':True,'limits':'UTF8 helper only; no claim that Python-str boundary can supply these invalid sequences to full extension. Native string helper contract and internal cleaning behavior controlled.'};pathlib.Path('docs/research/dedup-500/text108-controls-20261021/result.json').write_text(json.dumps(r,indent=2)+'\n');print(json.dumps(r,indent=2))
