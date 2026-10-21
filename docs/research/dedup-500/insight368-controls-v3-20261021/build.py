from pathlib import Path
R=Path(__file__).resolve().parents[4];D=Path(__file__).resolve().parent;C=R/'.dedup-work/insight368-20261021/Source/Platform'
strip=lambda s:'\n'.join(x for x in s.splitlines() if not x.startswith('#include'))
code='''// Authored platform/memory/alignment adapters; original class and methods follow.
#include <cstdint>
#include <cstring>
#include <cassert>
#include <iostream>
using UINT8=uint8_t;using UINT32=uint32_t;using VN_STATUS=int;using BOOL=bool;
#define VOID void
#define CONST const
#define IN
#define INOUT
#define VN_PARAM_CHECK 1
#define VN_SUCCESS 0
#define VN_ERROR_INVALIDARG -1
#define VN_ERROR_EXECUTION_FAILURE -2
#define VN_ERROR_OUTOFMEMORY -3
#define VN_ERROR_CAPACITY_LIMIT -4
#define VN_ERROR_INVALID_RESOURCE -5
#define VN_FAILED(x) ((x)!=0)
#define VN_ERR(...) ((void)0)
#define VN_WRN(...) ((void)0)
#define VN_MIN2(a,b) ((a)<(b)?(a):(b))
#define TRUE true
#define FALSE false
int vnPostError(int x){return x;}
UINT32 vnAlign(UINT32 x,UINT32 a){return (x+a-1)/a*a;}
void vnCopyMemory(void*d,const void*s,UINT32 n){std::memcpy(d,s,n);}
'''+strip((C/'vnBitStream.h').read_text())+'\n'+strip((C/'vnBitStream.cpp').read_text())+'''
unsigned reverse(unsigned x){unsigned v=0;for(int i=0;i<8;i++)v=(v<<1)|((x>>i)&1);return v;}
int main(){
 for(unsigned v=0;v<256;v++){
  CVBitStream s;s.ResizeCapacity(16);assert(s.WriteByte(v)==0);UINT8 out=0;
  assert(s.ReadByte(&out)==0 && out==v);
 }
 std::cout<<"PASS aligned byte roundtrip for all256 values\\n";
 for(unsigned v=0;v<256;v++){
  CVBitStream s;s.ResizeCapacity(16);s.WriteBit(1);s.WriteByte(v);s.Seek(1);UINT8 out=0;
  assert(s.ReadByte(&out)==0 && out==reverse(v));
 }
 std::cout<<"PASS unaligned ReadByte reverses all256 values relative to WriteByte\\n";
 for(unsigned v=0;v<256;v++){
  CVBitStream s;s.ResizeCapacity(16);s.WriteBit(1);s.WriteByte(v);s.Seek(1);UINT8 out=0;UINT32 n=1;
  assert(s.ReadBytes(&out,&n)==0 && n==1 && out==v);
 }
 std::cout<<"PASS unaligned bulk ReadBytes preserves all256 values\\n";
 CVBitStream s;s.ResizeCapacity(8);s.WriteByte(19);assert(s.WriteBit(1)==VN_ERROR_CAPACITY_LIMIT);
 UINT8 out=0;assert(s.ReadByte(&out)==0 && out==19 && s.IsEmpty());
 std::cout<<"PASS capacity refusal preserves stored byte and occupancy\\n";
}
'''
(D/'extracted.cpp').write_text('\n'.join(x.rstrip() for x in code.splitlines())+'\n')
