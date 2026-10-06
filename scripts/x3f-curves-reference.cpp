// Test-only noise-bank reference; luminance is an explicit test parameter.
#include <algorithm>
#include <cmath>
#include <cstdint>
#include <cstring>
#include <fstream>
#include <iterator>
#include <map>
#include <string>
#include <vector>
using Bytes=std::vector<unsigned char>;
static uint32_t word(const Bytes&b,size_t p){return b.at(p)|(uint32_t(b.at(p+1))<<8)|(uint32_t(b.at(p+2))<<16)|(uint32_t(b.at(p+3))<<24);}
static float real(const Bytes&b,size_t p){uint32_t bits=word(b,p);float f;std::memcpy(&f,&bits,4);return f;}
int main(int argc,char**argv){
 if(argc!=3 && argc!=4)return 2;
 double luminance=1.0;
 if(argc==4){std::ifstream tf(argv[3],std::ios::binary);tf.read(reinterpret_cast<char*>(&luminance),8);if(!tf||!std::isfinite(luminance)||luminance<=0)return 1;}
 std::ifstream in(argv[1],std::ios::binary);Bytes b{std::istreambuf_iterator<char>(in),{}};
 std::map<std::string,size_t> matrices;
 for(size_t p=0;p<b.size();){auto size=word(b,p+8),name=word(b,p+12),value=word(b,p+16);if(!size||p+size>b.size())return 1;
  if(std::memcmp(b.data()+p,"CMbM",4)==0)matrices[reinterpret_cast<const char*>(b.data()+p+name)]=p+word(b,p+value+8);
  p+=size;
 }
 float div[3],maximum=0;for(int c=0;c<3;++c)maximum=std::max(maximum,div[c]=real(b,matrices.at("AutoRGBNeutral")+c*4));for(auto&v:div)v/=maximum;
 float filter=real(b,matrices.at("ColumnFilter"));
 double color[3],chroma[3],color_max=0,chroma_max=0,combined=luminance;
 for(int c=0;c<3;++c){color[c]=real(b,matrices.at("ColorDQCamRGB")+c*4)/div[c];float dq=real(b,matrices.at("ChromaDQ")+c*4)/3;chroma[c]=dq/div[c];combined+=chroma[c];color_max=std::max(color_max,color[c]);chroma_max=std::max(chroma_max,chroma[c]);}
 std::ofstream out(argv[2],std::ios::binary);
 auto curve=[&](double maximum,double multiplier){
  double f=filter?filter:0.8;uint32_t size=uint32_t(4*std::acos(-1.0)*maximum/f);
  char header[4]={char(size),char(size>>8),char(size>>16),char(size>>24)};out.write(header,4);
  for(uint32_t i=0;i<size;++i){double x=i*f/maximum/4;int16_t value=int16_t((std::cos(x)+1)/2*std::tanh(i*f/multiplier)*multiplier+0.5);char bytes[2]={char(value),char(uint16_t(value)>>8)};out.write(bytes,2);}
 };
 for(int c=0;c<3;++c)curve(color_max,color[c]);for(int c=0;c<3;++c)curve(chroma_max,chroma[c]);curve(combined,combined);curve(combined*2,combined*2);
 return out?0:1;
}
