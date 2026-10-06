// Test-only sensor-stage arithmetic reference; not a display-color converter.
#include <algorithm>
#include <array>
#include <cmath>
#include <cstdint>
#include <cstring>
#include <fstream>
#include <iterator>
#include <map>
#include <string>
#include <vector>
using Bytes=std::vector<unsigned char>;
static Bytes read(const char*p){std::ifstream f(p,std::ios::binary);return {std::istreambuf_iterator<char>(f),{}};}
static uint32_t word(const Bytes&b,size_t p){return b.at(p)|(uint32_t(b.at(p+1))<<8)|(uint32_t(b.at(p+2))<<16)|(uint32_t(b.at(p+3))<<24);}
static float real(const Bytes&b,size_t p){uint32_t v=word(b,p);float f;std::memcpy(&f,&v,4);return f;}
int main(int argc,char**argv){
 if(argc!=5)return 2;
 auto sensor=read(argv[1]),camf=read(argv[2]),black=read(argv[3]);
 constexpr int w=2304,h=1531;
 if(sensor.size()!=size_t(w)*h*6||black.size()!=h*12)return 1;
 struct Matrix{size_t data;std::vector<unsigned> dims;};std::map<std::string,Matrix> matrices;
 for(size_t p=0;p<camf.size();){
  auto n=word(camf,p+8),name=word(camf,p+12),value=word(camf,p+16);
  if(!n||p+n>camf.size())return 1;
  if(std::memcmp(camf.data()+p,"CMbM",4)==0){
   Matrix m{p+word(camf,p+value+8),{}};
   for(unsigned i=0;i<word(camf,p+value+4);++i)m.dims.push_back(word(camf,p+value+12+i*12));
   matrices[reinterpret_cast<const char*>(camf.data()+p+name)]=m;
  }p+=n;
 }
 auto drift=matrices.at("DarkDrift"),poly=matrices.at("PostPolyMatrix"),gain=matrices.at("SpatialGain"),neutral=matrices.at("AutoRGBNeutral");
 float div[3],max=0;for(int c=0;c<3;++c)max=std::max(max,div[c]=real(camf,neutral.data+c*4));
 for(auto&v:div)v/=max;
 float cf=real(camf,matrices.at("ColumnFilter").data);
 unsigned gy=gain.dims.at(0),gx=gain.dims.at(1),spacing=(w+gx-2)/(gx-1);
 std::vector<std::array<float,3>> horizontal(gx);
 auto sample=[&](int y,int x,int c){size_t p=(size_t(y)*w+x)*6+c*2;return int16_t(uint16_t(sensor.at(p))|(uint16_t(sensor.at(p+1))<<8));};
 std::ofstream out(argv[4],std::ios::binary);
 for(int y=0;y<h;++y){
  float dr[3][2];for(int c=0;c<3;++c)for(int k=0;k<2;++k){float a=real(camf,drift.data+(c*2+k)*4),b=real(camf,drift.data+(6+c*2+k)*4);dr[c][k]=a+double(y)/double(h-1)*(b-a);}
  float fraction=double(y)/double(h-1)*(gy-1);unsigned top=unsigned(fraction);if(top==gy-1)--top;fraction-=top;
  for(unsigned x=0;x<gx;++x)for(int c=0;c<3;++c){float a=real(camf,gain.data+((top*gx+x)*3+c)*4),b=real(camf,gain.data+(((top+1)*gx+x)*3+c)*4);horizontal[x][c]=a*(1-fraction)+b*fraction;}
  int previous[3];for(int c=0;c<3;++c)previous[c]=sample(y,0,c);
  for(int x=0;x<w;++x){
   int p[3];int64_t terms[3][3];
   for(int c=0;c<3;++c){int current=sample(y,x,c);int64_t difference=current-previous[c];previous[c]=current;
    double correction=float(difference+((difference*difference)>>14))*cf-dr[c][1]-dr[c][0]*(float(x)/w-0.5)-real(black,(y*3+c)*4);
    p[c]=current+int(std::floor(double(correction)));
   }
   for(int c=0;c<3;++c){terms[0][c]=(int64_t(p[c])*p[c])>>14;terms[2][c]=(int64_t(p[c])*terms[0][c])>>14;terms[1][2-c]=(int64_t(p[(c+1)%3])*p[(c+2)%3])>>14;}
   for(int c=0;c<3;++c){float correction=0;
    for(int i=0;i<3;++i)for(int j=0;j<3;++j)correction+=real(camf,poly.data+(c*9+i*3+j)*4)*terms[i][j];
    float g=horizontal[x/spacing][c]*(spacing-x%spacing)+horizontal[x/spacing+1][c]*(x%spacing);
    int32_t value=int32_t(std::floor((p[c]+std::floor(double(correction)))*g/spacing/div[c]));value=std::min(value,32000);
    uint32_t bits=uint32_t(value);char bytes[4]={char(bits),char(bits>>8),char(bits>>16),char(bits>>24)};out.write(bytes,4);
   }
  }
 }
 return out?0:1;
}
