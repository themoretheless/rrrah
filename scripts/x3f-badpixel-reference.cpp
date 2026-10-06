// Test-only legacy bad-pixel arithmetic reference on pinned SD10 camera channels.
#include <cstdint>
#include <cstring>
#include <fstream>
#include <iterator>
#include <vector>
using Bytes=std::vector<unsigned char>;
static Bytes read(const char*p){std::ifstream f(p,std::ios::binary);return {std::istreambuf_iterator<char>(f),{}};}
static uint32_t word(const Bytes&b,size_t p){return b.at(p)|(uint32_t(b.at(p+1))<<8)|(uint32_t(b.at(p+2))<<16)|(uint32_t(b.at(p+3))<<24);}
int main(int argc,char**argv){
 if(argc!=4)return 2;
 auto input=read(argv[1]),camf=read(argv[2]);constexpr unsigned w=2304,h=1531;
 if(input.size()!=size_t(w)*h*12)return 1;
 std::vector<int32_t> pixels(w*h*3);for(size_t i=0;i<pixels.size();++i)pixels[i]=int32_t(word(input,i*4));
 size_t bad=0;unsigned count=0,left=0,top=0;bool origin=false;
 for(size_t p=0;p<camf.size();){
  auto n=word(camf,p+8),name=word(camf,p+12),value=word(camf,p+16);if(!n||p+n>camf.size())return 1;
  const char*title=reinterpret_cast<const char*>(camf.data()+p+name);
  if(std::strcmp(title,"BadPixels")==0){bad=p+word(camf,p+value+8);count=word(camf,p+value+12);}
  if(std::strcmp(title,"KeepImageArea")==0){size_t data=p+word(camf,p+value+8);left=word(camf,data);top=word(camf,data+4);origin=true;}
  p+=n;
 }
 if(!bad||!origin)return 1;
 const int dy[8]={-1,-1,-1,0,0,1,1,1},dx[8]={-1,0,1,-1,1,-1,0,1};
 for(unsigned i=0;i<count;++i){
  uint32_t code=word(camf,bad+i*4);int x=int((code>>8)&0xfff)-int(left),y=int(code>>20)-int(top);
  if(x<1||y<1||x>=int(w)-1||y>=int(h)-1)continue;
  int64_t sum[3]={};int n=0;
  for(int j=0;j<8;++j)if(code&(1u<<j)){
   for(int c=0;c<3;++c)sum[c]+=int16_t(pixels[((y+dy[j])*w+x+dx[j])*3+c]);++n;
  }
  if(n)for(int c=0;c<3;++c)pixels[(y*w+x)*3+c]=int32_t(sum[c]/n);
 }
 std::ofstream out(argv[3],std::ios::binary);
 for(auto v:pixels){uint32_t bits=uint32_t(v);char b[4]={char(bits),char(bits>>8),char(bits>>16),char(bits>>24)};out.write(b,4);}
 return out?0:1;
}
