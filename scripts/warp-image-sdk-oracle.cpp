// External complete GoPro DNG WarpRectilinear image opcode oracle.
#include "dng_lens_correction.h"
#include "dng_matrix.h"
#include "dng_host.h"
#include "dng_negative.h"
#include "dng_simple_image.h"
#include "dng_pixel_buffer.h"
#include "dng_tag_types.h"
#include "dng_rational.h"
#include <fstream>
#include <vector>
#include <string>
int main(int argc,char**argv) {
 if(argc!=2 && argc!=3)return 2;
 dng_host host; AutoPtr<dng_negative> negative(host.Make_dng_negative());
 AutoPtr<dng_image> image(new dng_simple_image(dng_rect(0,0,32,48),3,ttFloat,host.Allocator()));
 std::vector<float> data(32*48*3);
 for(int y=0;y<32;y++)for(int x=0;x<48;x++)for(int c=0;c<3;c++)data[(y*48+x)*3+c]=float(((x*17+y*31+c*43)%251)/250.0);
 bool hdr=argc==3 && std::string(argv[2])=="hdr";
 if(hdr)for(float &v:data)v=v*4.0f-0.5f;
 dng_pixel_buffer buf;buf.fArea=dng_rect(0,0,32,48);buf.fPlane=0;buf.fPlanes=3;buf.fRowStep=48*3;buf.fColStep=3;buf.fPlaneStep=1;buf.fPixelType=ttFloat;buf.fPixelSize=4;buf.fData=data.data();buf.fDirty=true;
 image->Put(buf);
 if(argc==3 && std::string(argv[2])=="aspect")negative->SetDefaultScale(dng_urational(3,2),dng_urational(1,1));
 unsigned planes=argc==3?3:1; negative->SetColorChannels(3);
 dng_vector rad[3],tan[3];
 for(unsigned p=0;p<planes;p++){rad[p].SetIdentity(4);tan[p].SetIdentity(2);
 rad[p][0]=.97+.01*p;rad[p][1]=.05-.02*p;rad[p][2]=-.01;rad[p][3]=.002;tan[p][0]=.003+.001*p;tan[p][1]=-.004+.002*p;}
 dng_warp_params_rectilinear params(planes,rad,tan,dng_point_real64(.6,.4));
 dng_opcode_WarpRectilinear opcode(params,0);opcode.Apply(host,*negative,image);
 image->Get(buf);
 std::ofstream out(argv[1],std::ios::binary);out.write(reinterpret_cast<const char*>(data.data()),data.size()*4);
}
