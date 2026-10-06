// Test-only Auto transform reference with explicit identity XYZ target.
#include <algorithm>
#include <cstdint>
#include <cstring>
#include <fstream>
#include <iterator>
#include <map>
#include <string>
#include <vector>
using Bytes=std::vector<unsigned char>;
static uint32_t word(const Bytes&b,size_t p){return b.at(p)|(uint32_t(b.at(p+1))<<8)|(uint32_t(b.at(p+2))<<16)|(uint32_t(b.at(p+3))<<24);}
static float real(const Bytes&b,size_t p){auto bits=word(b,p);float v;std::memcpy(&v,&bits,4);return v;}
int main(int argc,char**argv){
 if(argc!=3 && argc!=4)return 2;std::ifstream in(argv[1],std::ios::binary);Bytes b{std::istreambuf_iterator<char>(in),{}};std::map<std::string,size_t> matrices;
 for(size_t p=0;p<b.size();){auto n=word(b,p+8),name=word(b,p+12),value=word(b,p+16);if(!n||p+n>b.size())return 1;
  if(std::memcmp(b.data()+p,"CMbM",4)==0)matrices[reinterpret_cast<const char*>(b.data()+p+name)]=p+word(b,p+value+8);p+=n;
 }
 float reference_target[3][3]={{1,0,0},{0,1,0},{0,0,1}};
 if(argc==4){float selected[3][3]={{1.4032f,-0.2231f,-0.1016f},{-0.5263f,1.4816f,0.017f},{-0.0112f,0.0183f,0.9113f}};std::memcpy(reference_target,selected,sizeof selected);}
 float neutral[3],maximum=0;for(int c=0;c<3;++c)maximum=std::max(maximum,neutral[c]=real(b,matrices.at("AutoRGBNeutral")+c*4));for(auto&v:neutral)v/=maximum;
 float combined[3][3]={},target[3][3]={},last[3][3],output[3][3]={};
 for(int i=0;i<3;++i)for(int j=0;j<3;++j)for(int c=0;c<3;++c)
  combined[i][j]+=real(b,matrices.at("WBCorrection_Identity")+(i*3+c)*4)*real(b,matrices.at("CamToXYZ_Flash")+(c*3+j)*4);
 for(int i=0;i<3;++i)for(int j=0;j<3;++j)for(int c=0;c<3;++c)target[i][j]+=reference_target[i][c]*combined[c][j]*neutral[j];
 double sums[3];for(int i=0;i<3;++i)sums[i]=target[i][0]+target[i][1]+target[i][2];
 double luminance=(6*sums[0]+11*sums[1]+3*sums[2])/20;
 for(int i=0;i<3;++i)for(int j=0;j<3;++j)last[i][j]=target[i][j]*luminance/sums[i];
 for(int i=0;i<3;++i)for(int j=0;j<3;++j)for(int c=0;c<3;++c)output[i][j]+=(i==c?32:-1)*last[c][j]/30;
 std::ofstream out(argv[2],std::ios::binary);out.write(reinterpret_cast<char*>(&luminance),8);out.write(reinterpret_cast<char*>(output),36);return out?0:1;
}
