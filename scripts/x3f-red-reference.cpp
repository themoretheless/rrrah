// Test-only reference with full precomputed horizontal rows, independent of Rust ring storage.
#include <algorithm>
#include <cstdint>
#include <fstream>
#include <vector>
int main(int argc,char**argv){
 if(argc!=3)return 2;
 constexpr int w=2304,h=1531;
 std::vector<int32_t> pixels(w*h*3);
 std::ifstream in(argv[1],std::ios::binary);in.read(reinterpret_cast<char*>(pixels.data()),pixels.size()*4);if(!in)return 1;
 std::vector<int64_t> horizontal(w*h);
 for(int y=0;y<h;++y)for(int x=2;x<w-2;++x){
  auto at=[&](int col){return int64_t(pixels[(y*w+col)*3]);};
  horizontal[y*w+x]=(at(x)*6+(at(x-1)+at(x+1))*4+at(x-2)+at(x+2)+8)>>4;
 }
 for(int y=2;y<h-2;++y){
  int64_t previous=0;
  for(int x=2;x<w-2;++x){
   auto at=[&](int row){return horizontal[row*w+x];};
   int64_t smooth=(6*at(y)+4*(at(y-1)+at(y+1))+at(y-2)+at(y+2)+8)>>4;
   if(x==2)previous=smooth;
   int64_t red=pixels[(y*w+x)*3];
   pixels[(y*w+x)*3]=int32_t(std::min<int64_t>(red+((red-((smooth*7+previous)>>3))>>3),32000));
   previous=smooth;
  }
 }
 std::ofstream out(argv[2],std::ios::binary);out.write(reinterpret_cast<char*>(pixels.data()),pixels.size()*4);return out?0:1;
}
