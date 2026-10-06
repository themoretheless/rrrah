// Test-only independent full-frame color arithmetic, explicit target/curve bank.
#include <array>
#include <cmath>
#include <cstdint>
#include <fstream>
#include <vector>
int main(int argc,char**argv){
 if(argc!=5)return 2;
 std::vector<int32_t> pixels(2304*1531*3);
 std::ifstream input(argv[1],std::ios::binary);input.read(reinterpret_cast<char*>(pixels.data()),pixels.size()*4);if(!input)return 1;
 std::ifstream cf(argv[2],std::ios::binary);std::array<std::vector<int16_t>,3> curves;
 for(auto &curve:curves){uint32_t n;cf.read(reinterpret_cast<char*>(&n),4);if(!cf||n>32767)return 1;curve.resize(n);cf.read(reinterpret_cast<char*>(curve.data()),n*2);if(!cf)return 1;}
 double luminance;float transform[3][3];std::ifstream tf(argv[3],std::ios::binary);tf.read(reinterpret_cast<char*>(&luminance),8);tf.read(reinterpret_cast<char*>(transform),36);if(!tf)return 1;
 auto apply=[&](int c,int v){unsigned magnitude=unsigned(v<0?-int64_t(v):v);if(magnitude>=curves[c].size())return 0;return (v<0?-1:1)*int(curves[c][magnitude]);};
 for(size_t p=0;p<pixels.size();p+=3){
  int channel[3];for(int c=0;c<3;++c)channel[c]=pixels[p+c]-apply(c,pixels[p+c]);
  int mean=(channel[0]+2*channel[1]+channel[2])>>2;
  for(int c=0;c<3;++c)channel[c]-=apply(c,channel[c]-mean);
  for(int c=0;c<3;++c){double sum=0;for(int i=0;i<3;++i)sum+=transform[c][i]*channel[i];if(sum<0)sum=0;if(sum>24000)sum=24000;pixels[p+c]=int(sum+0.5);}
 }
 std::ofstream out(argv[4],std::ios::binary);out.write(reinterpret_cast<char*>(pixels.data()),pixels.size()*4);return out?0:1;
}
