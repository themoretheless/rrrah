// Test-only full-buffer reference for the legacy hue passes.
#include <cstdint>
#include <fstream>
#include <iterator>
#include <vector>
using Bytes=std::vector<unsigned char>;
static uint32_t word(const Bytes&b,size_t p){return b.at(p)|(uint32_t(b.at(p+1))<<8)|(uint32_t(b.at(p+2))<<16)|(uint32_t(b.at(p+3))<<24);}
int main(int argc,char**argv){
 if(argc!=4 && argc!=5)return 2;constexpr int w=2304,h=1531;
 std::vector<int32_t> pixels(w*h*3);std::ifstream in(argv[1],std::ios::binary);in.read(reinterpret_cast<char*>(pixels.data()),pixels.size()*4);if(!in)return 1;
 std::ifstream cf(argv[2],std::ios::binary);Bytes curves{std::istreambuf_iterator<char>(cf),{}};size_t p=0;uint32_t size=0;
 for(int i=0;i<8;++i){size=word(curves,p);p+=4;if(i==(argc==5?6:7))break;p+=size*2;}
 auto apply=[&](int v){unsigned i=unsigned(v<0?-int64_t(v):v);if(i>=size)return 0;size_t q=p+i*2;int value=int16_t(uint16_t(curves.at(q))|(uint16_t(curves.at(q+1))<<8));return v<0?-value:value;};
 std::vector<int64_t> horizontal(w*h*3);
 for(int y=0;y<h;++y)for(int x=2;x<w-2;++x)for(int c=0;c<3;++c)
  horizontal[(y*w+x)*3+c]=argc==5?
   (int64_t(pixels[(y*w+x-2)*3+c])+pixels[(y*w+x-1)*3+c]+pixels[(y*w+x)*3+c]+pixels[(y*w+x+1)*3+c]+pixels[(y*w+x+2)*3+c]+2)>>2:
   (int64_t(pixels[(y*w+x-1)*3+c])+2*int64_t(pixels[(y*w+x)*3+c])+pixels[(y*w+x+1)*3+c]+2)>>2;
 for(int y=2;y<h-2;++y)for(int x=2;x<w-2;++x){
  if(argc==5){
   int64_t total[3]={},all=375,sum=60;
   for(int c=0;c<3;++c){for(int row=y-2;row<=y+2;++row)total[c]+=horizontal[(row*w+x)*3+c];all+=total[c];sum+=pixels[(y*w+x)*3+c];}
   if(sum<0)sum=0;int64_t ratio=all>375?(sum<<16)/all:sum*174;
   for(int c=0;c<3;++c)pixels[(y*w+x)*3+c]+=apply(int(((ratio*total[c]+32768)>>16)-pixels[(y*w+x)*3+c]));
   continue;
  }
  int dev[3];for(int c=0;c<3;++c){auto at=[&](int row){return horizontal[(row*w+x)*3+c];};int average=int((at(y-1)+2*at(y)+at(y+1))>>2);dev[c]=-apply(pixels[(y*w+x)*3+c]-average);}
  int common=(dev[0]+dev[1]+dev[2])>>3;for(int c=0;c<3;++c)pixels[(y*w+x)*3+c]+=dev[c]-common;
 }
 std::ofstream out(argv[3],std::ios::binary);out.write(reinterpret_cast<char*>(pixels.data()),pixels.size()*4);return out?0:1;
}
