// Independent Little CMS 2 oracle: ICC_PROFILE P6_16BIT_FILE.
// Samples the same 32x32 nearest-position grid as raw_raster_probe.
// Reference diagnostics only; not linked into the library.
#include <lcms2.h>
#include <fstream>
#include <iostream>
#include <vector>
#include <iomanip>
int main(int argc,char**argv){
 if(argc!=3&&argc!=4)return 2;
 std::ofstream grid; if(argc==4)grid.open(argv[3],std::ios::binary);
 auto source=cmsOpenProfileFromFile(argv[1],"r");
 cmsCIExyY white={0.3127,0.3290,1};
 cmsCIExyYTRIPLE primaries={{0.64,0.33,1},{0.30,0.60,1},{0.15,0.06,1}};
 auto curve=cmsBuildGamma(nullptr,1);cmsToneCurve* curves[3]={curve,curve,curve};
 auto target=cmsCreateRGBProfile(&white,&primaries,curves);
 auto transform=cmsCreateTransform(source,TYPE_RGB_DBL,target,TYPE_RGB_DBL,INTENT_RELATIVE_COLORIMETRIC,cmsFLAGS_NOOPTIMIZE|cmsFLAGS_NOCACHE);
 if(!transform)return 3;
 std::ifstream stream(argv[2],std::ios::binary);std::string magic;unsigned w,h,max;stream>>magic>>w>>h>>max;stream.get();
 if(magic!="P6"||max!=65535)return 4;
 std::vector<unsigned char> pixels(size_t(w)*h*6);stream.read(reinterpret_cast<char*>(pixels.data()),pixels.size());
 double mean[3]={};unsigned above=0,below=0;
 for(unsigned y=0;y<32;y++)for(unsigned x=0;x<32;x++){
  size_t i=(size_t((2*y+1)*h/64)*w+(2*x+1)*w/64)*6;double input[3],out[3];
  for(unsigned c=0;c<3;c++)input[c]=double(pixels[i+2*c]*256+pixels[i+2*c+1])/65535;
  cmsDoTransform(transform,input,out,1);
  if(grid.is_open()){float rgba[4]={float(out[0]),float(out[1]),float(out[2]),1.f};grid.write(reinterpret_cast<const char*>(rgba),sizeof(rgba));}
  for(unsigned c=0;c<3;c++){mean[c]+=out[c]/1024;above+=out[c]>1;below+=out[c]<0;}
 }
 std::cout<<std::setprecision(15)<<"{\"linear_means\":["<<mean[0]<<","<<mean[1]<<","<<mean[2]<<"],\"above_one\":"<<above<<",\"negative\":"<<below<<"}\n";
 cmsDeleteTransform(transform);cmsCloseProfile(source);cmsCloseProfile(target);cmsFreeToneCurve(curve);
}
