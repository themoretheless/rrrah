from pathlib import Path
R=Path(__file__).resolve().parents[4];D=Path(__file__).resolve().parent;C=R/'.dedup-work/insight368-20261021'
s=(C/'Source/Imagine/vnImageDesaturate.cpp').read_text();fn=s[s.index('VN_STATUS vnDesaturateLine('):s.index('VN_STATUS vnDesaturateImage(')]
license_text=(C/'Source/Platform/vnPlatform.h').read_text().split('// Description:')[0]
code=license_text+'''// Authored type/error adapters; original function body follows.
#include <cstdint>
#include <cassert>
#include <iostream>
using UINT8=uint8_t;using UINT32=uint32_t;using INT16=int16_t;using VN_STATUS=int;
#define IN
#define VN_PARAM_CHECK 1
#define VN_ERROR_INVALIDARG -1
#define VN_SUCCESS 0
int vnPostError(int x){return x;}
'''+fn+'''
int main(){
 for(unsigned v=0;v<256;v++){
  UINT8 rgb[3]={UINT8(v),UINT8(v),UINT8(v)}, out=0;
  assert(vnDesaturateLine(rgb,1,&out)==0);
  assert(out==(v==0?0:v-1));
 }
 std::cout<<"PASS all256 neutral levels:0 maps0;1..255 each reduced by1\\n";
 UINT8 rgb[3]={255,255,255},out=0;
 assert(vnDesaturateLine(rgb,1,&out)==0 && out==254);
 std::cout<<"PASS opaque white converts254 rather than255\\n";
 assert(vnDesaturateLine(nullptr,1,&out)==-1);
 assert(vnDesaturateLine(rgb,0,&out)==-1);
 assert(vnDesaturateLine(rgb,1,nullptr)==-1);
 std::cout<<"PASS null pointers and zero pixel count explicitly refused\\n";
}
'''
(D/'extracted.cpp').write_text('\n'.join(x.rstrip() for x in code.splitlines())+'\n')
