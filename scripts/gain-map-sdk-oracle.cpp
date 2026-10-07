// External pinned GoPro DNG SDK interpolation oracle; never linked into Rrrah.
#include "dng_gain_map.h"
#include "dng_memory.h"
#include <fstream>
int main(int argc,char**argv) {
 if(argc!=2)return 2;
 dng_gain_map map(gDefaultDNGMemoryAllocator,dng_point(3,4),dng_point_real64(.3,.2),dng_point_real64(.1,-.05),1);
 for(unsigned y=0;y<3;y++)for(unsigned x=0;x<4;x++)map.Entry(y,x,0)=float(1.0+.13*y+.07*x+.03*x*y);
 std::ofstream out(argv[1],std::ios::binary);
 for(int y=0;y<20;y++)for(int x=0;x<23;x++) {
  float gain=map.Interpolate(y,x,0,dng_rect(0,0,20,23));
  out.write(reinterpret_cast<const char*>(&gain),4);
 }
}
