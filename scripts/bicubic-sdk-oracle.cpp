// External GoPro DNG SDK 2D warp-weight oracle; output is little-endian f32 on arm64.
#include "dng_resample.h"
#include "dng_memory.h"
#include <fstream>
int main(int argc,char**argv) {
 if(argc!=2)return 2;
 dng_resample_weights_2d weights;
 weights.Initialize(dng_resample_bicubic::Get(),gDefaultDNGMemoryAllocator);
 if(weights.Width()!=4 || kResampleSubsampleCount2D!=32)return 3;
 std::ofstream out(argv[1],std::ios::binary);
 for(int y=0;y<32;y++)for(int x=0;x<32;x++) {
  auto p=weights.Weights32(dng_point(y,x));
  out.write(reinterpret_cast<const char*>(p),16*sizeof(float));
 }
}
