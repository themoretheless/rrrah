// Test-only legacy calibrated neutral arithmetic. Never linked into production.
#include <cstdint>
#include <cstring>
#include <fstream>
#include <iterator>
#include <map>
#include <string>
#include <vector>
using Bytes = std::vector<unsigned char>;
static uint32_t word(const Bytes& b, size_t p) {
 return b.at(p) | uint32_t(b.at(p+1))<<8 | uint32_t(b.at(p+2))<<16 | uint32_t(b.at(p+3))<<24;
}
static float real(const Bytes& b, size_t p) { auto bits=word(b,p); float v; std::memcpy(&v,&bits,4); return v; }
int main(int argc, char** argv) {
 if(argc!=4) return 2;
 std::ifstream input(argv[1],std::ios::binary); Bytes b{std::istreambuf_iterator<char>(input),{}};
 std::map<std::string,size_t> matrices;
 std::map<std::string,std::map<std::string,std::string>> tables;
 for(size_t p=0;p<b.size();) {
  auto n=word(b,p+8),name=word(b,p+12),value=word(b,p+16);
  if(n<20 || p+n>b.size()) return 1;
  std::string key(reinterpret_cast<const char*>(b.data()+p+name));
  if(std::memcmp(b.data()+p,"CMbM",4)==0) matrices[key]=p+word(b,p+value+8);
  if(std::memcmp(b.data()+p,"CMbP",4)==0) {
   auto count=word(b,p+value); size_t base=p+word(b,p+value+4);
   for(unsigned i=0;i<count;++i) {
    auto a=base+word(b,p+value+8+i*8),v=base+word(b,p+value+12+i*8);
    tables[key][reinterpret_cast<const char*>(b.data()+a)]=reinterpret_cast<const char*>(b.data()+v);
   }
  }
  p+=n;
 }
 std::string mode(argv[2]); float neutral[3];
 if(tables.at("IncludeBlocks").count(mode+"RGBNeutral")) {
  for(int c=0;c<3;++c) neutral[c]=real(b,matrices.at(mode+"RGBNeutral")+c*4);
 } else {
  size_t xyz=matrices.at(tables.at("WhiteBalanceIlluminants").at(mode));
  size_t correction=matrices.at(tables.at("WhiteBalanceCorrections").at(mode));
  float last[3][3]={},diag[3][3];
  for(int i=0;i<3;++i) for(int j=0;j<3;++j) for(int c=0;c<3;++c)
   last[i][j]+=real(b,correction+(i*3+c)*4)*real(b,xyz+(c*3+j)*4);
  for(int i=0;i<3;++i) for(int c=0;c<3;++c)
   diag[c][i]=last[(i+1)%3][(c+1)%3]*last[(i+2)%3][(c+2)%3]-last[(i+1)%3][(c+2)%3]*last[(i+2)%3][(c+1)%3];
  for(int c=0;c<3;++c) neutral[c]=diag[c][0]*0.3127+diag[c][1]*0.329+diag[c][2]*0.3583;
 }
 std::ofstream output(argv[3],std::ios::binary); output.write(reinterpret_cast<char*>(neutral),12); return output?0:1;
}
