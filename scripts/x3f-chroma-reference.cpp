// Test-only full-frame quarter-guide and final chroma arithmetic reference.
#include <array>
#include <cstdint>
#include <fstream>
#include <vector>
int main(int argc,char**argv){
 if(argc!=5)return 2;constexpr int w=2304,h=1531,qw=w/4,qh=h/4;
 std::vector<int32_t> pixels(w*h*3);std::ifstream in(argv[1],std::ios::binary);in.read(reinterpret_cast<char*>(pixels.data()),pixels.size()*4);if(!in)return 1;
 std::array<std::vector<int16_t>,6> curves;std::ifstream cf(argv[2],std::ios::binary);
 for(auto &curve:curves){uint32_t n;cf.read(reinterpret_cast<char*>(&n),4);if(!cf||n>32767)return 1;curve.resize(n);cf.read(reinterpret_cast<char*>(curve.data()),n*2);if(!cf)return 1;}
 auto apply=[&](int c,int v){unsigned n=unsigned(v<0?-int64_t(v):v);if(n>=curves[c+3].size())return 0;return(v<0?-1:1)*int(curves[c+3][n]);};
 std::vector<int16_t> guide(qw*qh*3);
 for(int y=qh-1;y>=0;--y)for(int x=0;x<qw;++x)for(int c=0;c<3;++c){int64_t sum=0;for(int dy=0;dy<4;++dy)for(int dx=0;dx<4;++dx)sum+=pixels[((y*4+dy)*w+x*4+dx)*3+c];guide[(y*qw+x)*3+c]=y==qh-1?sum>>4:(guide[((y+1)*qw+x)*3+c]*1840+sum*141+2048)>>12;}
 std::ofstream gf(argv[3],std::ios::binary);gf.write(reinterpret_cast<char*>(guide.data()),guide.size()*2);if(!gf)return 1;
 // Store full intermediate frames, independent of the native row scratch layout.
 std::vector<int64_t> reverse(w*(h&~3)*3),forward(reverse.size()),vertical(reverse.size());
 for(int y=0;y<(h&~3);++y){
  for(int c=0;c<3;++c){int64_t prev=0;for(int x=w-1;x>=0;--x){prev=(guide[((y/4)*qw+x/4)*3+c]*1485+prev*6707+4096)>>13;reverse[(y*w+x)*3+c]=prev;}}
  for(int c=0;c<3;++c){int64_t prev=0;for(int x=0;x<w;++x){size_t i=(y*w+x)*3+c;prev=(reverse[i]*1485+prev*6707+4096)>>13;forward[i]=prev;vertical[i]=y==0?prev:(vertical[i-w*3]*6707+prev*1485+4096)>>13;}}
  for(int x=0;x<w;++x){size_t p=(y*w+x)*3;int64_t denominator=30,signal=30;for(int c=0;c<3;++c){denominator+=vertical[p+c];signal+=pixels[p+c];}int64_t ratio=(signal<<16)/denominator;int correction[3],common=0;for(int c=0;c<3;++c){correction[c]=apply(c,int(((vertical[p+c]*ratio+32768)>>16)-pixels[p+c]));common+=correction[c];}common>>=3;for(int c=0;c<3;++c){int value=pixels[p+c]+correction[c]-common;pixels[p+c]=value<0?0:value;}}
 }
 std::ofstream out(argv[4],std::ios::binary);out.write(reinterpret_cast<char*>(pixels.data()),pixels.size()*4);return out?0:1;
}
