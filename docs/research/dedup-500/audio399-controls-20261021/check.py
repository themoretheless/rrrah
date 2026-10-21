from pathlib import Path
import subprocess,json,hashlib
C=Path('.dedup-work/audio399-20261021');src=(C/'AudioHash.c').read_text()
def function(name):
 start=src.index(name+'(');start=src.rfind('\n',0,start)+1;brace=src.index('{',start);depth=1;i=brace+1
 while depth:
  depth+=(src[i]=='{')-(src[i]=='}');i+=1
 return src[start:i]
selected='\n'.join(function(n) for n in ['ph_bitcount','ph_compare_blocks','ph_audiohash_distance_ber'])
harness='''
#include <stdio.h>
#include <stdint.h>
#include <stddef.h>
#include <math.h>
#include "AudioHash.h"
'''+selected+'''
int main(void){
 unsigned int a=0,b=0x80000000u,t=0x80000000u;
 float distance=ph_compare_blocks(&a,NULL,&b,1);
 float masked=ph_compare_blocks(&a,&t,&b,1);
 float empty=ph_compare_blocks(&a,NULL,&b,0);
 if(ph_bitcount(b)!=32 || distance!=1.0f || masked!=0 || !isnan(empty)) return 1;
 printf("{\\"highest_single_bit_count\\":%d,\\"expected_popcount\\":1,\\"actual_distance\\":%.8f,\\"expected_distance\\":0.03125,\\"masked_distance\\":%.1f,\\"zero_length_nan\\":true}\\n",ph_bitcount(b),distance,masked);
}
'''
p=C/'controls.c';p.write_text(harness);subprocess.run(['cc','-I',str(C),str(p),'-lm','-o',str(C/'controls')],check=True);result=json.loads(subprocess.check_output([str(C/'controls')],text=True));result.update(source_sha256=hashlib.sha256(src.encode()).hexdigest(),method='Exact original three function bodies extracted, compiled with original header; no FFT/hash generation/native pHash/ASAN.',original_full_suite_run=False);Path('docs/research/dedup-500/audio399-controls-20261021/result.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
