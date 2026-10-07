// External GoPro DNG SDK radial/tangential evaluation oracle.
#include "dng_lens_correction.h"
#include "dng_matrix.h"
#include <fstream>
#include <cmath>
#include <algorithm>
int main(int argc,char**argv) {
 if(argc!=2)return 2;
 dng_vector rad[1],tan[1];rad[0].SetIdentity(4);tan[0].SetIdentity(2);
 rad[0][0]=.97;rad[0][1]=.05;rad[0][2]=-.01;rad[0][3]=.002;
 tan[0][0]=.003;tan[0][1]=-.004;
 dng_warp_params_rectilinear params(1,rad,tan,dng_point_real64(.6,.4));
 const double cx=16,cy=18,radius=std::hypot(24.,18.);
 std::ofstream out(argv[1],std::ios::binary);
 for(int y=0;y<30;y+=2)for(int x=0;x<40;x+=2) {
  double dx=(x-cx)/radius,dy=(y-cy)/radius,r2=std::min(dx*dx+dy*dy,1.);
  double ratio=params.EvaluateRatio(0,r2);
  auto t=params.EvaluateTangential(0,r2,dng_point_real64(dy,dx),dng_point_real64(dy*dy,dx*dx));
  double result[2]={cx+radius*(dx*ratio+t.h),cy+radius*(dy*ratio+t.v)};
  out.write(reinterpret_cast<const char*>(result),sizeof(result));
 }
}
