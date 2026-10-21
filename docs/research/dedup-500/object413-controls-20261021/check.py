import pathlib,subprocess,json,hashlib
p=pathlib.Path('.dedup-work/object413-20261021/src/PerceptualHashObjectDetector.cpp');s=p.read_text()
def extract(signature):
 start=s.index(signature);i=s.index('{',start);level=1;j=i+1
 while level:
  level+=(s[j]=='{')-(s[j]=='}');j+=1
 return s[start:j]
parts=[extract('static void _getNoise32('),extract('void getNoise32('),extract('void getMeanNoise(')]
out=pathlib.Path('.dedup-work/object413-controls-20261021');out.mkdir(exist_ok=True)
code='#include <cstdio>\n#include <cstring>\n'+ '\n'.join(parts)+'''
int main(){unsigned char values[256];int checked=0,missing=0;
for(unsigned long seed=0;seed<256;seed++)for(int radius=0;radius<=3;radius++){
 memset(values,0,sizeof(values));getNoise32(seed,values,1,radius,8);
 for(unsigned long q=0;q<256;q++){int d=__builtin_popcountl(seed^q);bool expected=d>0&&d<=radius;if(bool(values[q])!=expected)return 2;}
 if(!values[seed])missing++;checked++;
}
memset(values,0,sizeof(values));getMeanNoise(values,100,10);int below=0,above=0;
for(int i=90;i<100;i++)below+=values[i]!=0;
for(int i=101;i<=110;i++)above+=values[i]!=0;
if(below!=10||above!=0)return 3;
printf("{\\\"hash_cases\\\":%d,\\\"seed_missing_cases\\\":%d,\\\"mean_below_marked\\\":%d,\\\"mean_above_marked\\\":%d}\\n",checked,missing,below,above);
}
'''
# C++ harness string is intentionally kept literal, then adjusted for JSON quotes.
code=code.replace('\\\"','\\"').replace('\\n",checked','\\n",checked')
(out/'control.cpp').write_text(code)
r=subprocess.run(['c++','-std=c++11',str(out/'control.cpp'),'-o',str(out/'control')],capture_output=True,text=True);assert r.returncode==0,r.stderr
r=subprocess.run([str(out/'control')],capture_output=True,text=True);assert r.returncode==0,r.stderr
v=json.loads(r.stdout);v.update({'source_sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'method':'Three original unchanged C++ function bodies brace-extracted and compiled with Apple C++ compiler; no OpenCV/image pipeline, pHash library, binary archive, camera or ASAN execution.','oracle':'All256 seed values, radii0..3, all256 query values compared with independent XOR builtin popcount for nonzero distances; exact seed separately checked.','finding':'All nonzero-distance neighbors correct in8-bit controls, but seed never included. Mean100 noise10 marks lower90..99 only; upper101..110 all absent.','limits':'No invalid dimensions/bit widths, float-image pointer reinterpretation or full detector runtime executed.'})
pathlib.Path('docs/research/dedup-500/object413-controls-20261021/result.json').write_text(json.dumps(v,indent=2)+'\n');print(json.dumps(v,indent=2))
