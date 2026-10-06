// Test-only pinned SD10 highlight arithmetic reference.
#include <algorithm>
#include <cstdint>
#include <cstring>
#include <fstream>
#include <iterator>
#include <vector>
using Bytes=std::vector<unsigned char>;
static Bytes read(const char*p){std::ifstream f(p,std::ios::binary);return {std::istreambuf_iterator<char>(f),{}};}
static uint32_t word(const Bytes&b,size_t p){return b.at(p)|(uint32_t(b.at(p+1))<<8)|(uint32_t(b.at(p+2))<<16)|(uint32_t(b.at(p+3))<<24);}
int main(int argc,char**argv){
 if(argc!=4)return 2;auto input=read(argv[1]),camf=read(argv[2]);if(input.size()%12)return 1;
 size_t saturation=0,neutral=0;
 for(size_t p=0;p<camf.size();){unsigned n=word(camf,p+8),name=word(camf,p+12),value=word(camf,p+16);if(!n||p+n>camf.size())return 1;
  auto title=reinterpret_cast<const char*>(camf.data()+p+name);size_t data=p+word(camf,p+value+8);
  if(std::strcmp(title,"SaturationLevel")==0)saturation=data;
  if(std::strcmp(title,"AutoRGBNeutral")==0)neutral=data;
  p+=n;
 }if(!saturation||!neutral)return 1;
 float div[3],maximum=0;for(int c=0;c<3;++c){auto bits=word(camf,neutral+c*4);std::memcpy(&div[c],&bits,4);maximum=std::max(maximum,div[c]);}
 int minimum=65535;for(int c=0;c<3;++c){div[c]/=maximum;unsigned sat=camf.at(saturation+c*2)|(unsigned(camf.at(saturation+c*2+1))<<8);minimum=std::min(minimum,int(sat/div[c]));}
 int limit=minimum*9>>4;std::ofstream out(argv[3],std::ios::binary);
 for(size_t p=0;p<input.size();p+=12){int v[3];for(int c=0;c<3;++c)v[c]=int32_t(word(input,p+c*4));
  if(v[0]>limit&&v[1]>limit&&v[2]>limit){int low=std::min({v[0],v[1],v[2]}),high=std::max({v[0],v[1],v[2]});
   if(low>=limit*2)for(auto&x:v)x=high;else{int weight=16384-((low-limit)<<14)/limit;weight=16384-(weight*weight>>14);weight=weight*weight>>14;for(auto&x:v)x+=(high-x)*weight>>14;}
  }
  for(auto x:v){uint32_t bits=uint32_t(x);char b[4]={char(bits),char(bits>>8),char(bits>>16),char(bits>>24)};out.write(b,4);}
 }return out?0:1;
}
