// External complete GoPro DNG WarpRectilinear image opcode oracle.
#include "dng_lens_correction.h"
#include "dng_matrix.h"
#include "dng_host.h"
#include "dng_negative.h"
#include "dng_simple_image.h"
#include "dng_pixel_buffer.h"
#include "dng_tag_types.h"
#include <fstream>
#include <vector>
int main(int argc,char**argv) {
 if(argc!=2)return 2;
 dng_host host; AutoPtr<dng_negative> negative(host.Make_dng_negative());
 AutoPtr<dng_image> image(new dng_simple_image(dng_rect(0,0,32,48),3,ttFloat,host.Allocator()));
 std::vector<float> data(32*48*3);
 for(int y=0;y<32;y++)for(int x=0;x<48;x++)for(int c=0;c<3;c++)data[(y*48+x)*3+c]=float(((x*17+y*31+c*43)%251)/250.0);
 dng_pixel_buffer buf;buf.fArea=dng_rect(0,0,32,48);buf.fPlane=0;buf.fPlanes=3;buf.fRowStep=48*3;buf.fColStep=3;buf.fPlaneStep=1;buf.fPixelType=ttFloat;buf.fPixelSize=4;buf.fData=data.data();buf.fDirty=true;
 image->Put(buf);
 dng_vector rad[1],tan[1];rad[0].SetIdentity(4);tan[0].SetIdentity(2);
 rad[0][0]=.97;rad[0][1]=.05;rad[0][2]=-.01;rad[0][3]=.002;tan[0][0]=.003;tan[0][1]=-.004;
 dng_warp_params_rectilinear params(1,rad,tan,dng_point_real64(.6,.4));
 dng_opcode_WarpRectilinear opcode(params,0);opcode.Apply(host,*negative,image);
 image->Get(buf);
 std::ofstream out(argv[1],std::ios::binary);out.write(reinterpret_cast<const char*>(data.data()),data.size()*4);
}
