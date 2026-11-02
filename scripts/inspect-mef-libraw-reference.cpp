// Test/reference tool only; not linked by any production crate.
#include <libraw/libraw.h>
#include <iostream>
#include <fstream>
int main(int argc,char**argv){if(argc!=3)return 2;LibRaw r;int e=r.open_file(argv[1]);if(e){std::cerr<<libraw_strerror(e);return 3;}
std::cout<<"libraw="<<LibRaw::version()<<"\n";
auto&s=r.imgdata.sizes;auto&c=r.imgdata.color;
std::cout<<"make="<<r.imgdata.idata.make<<" model="<<r.imgdata.idata.model<<" raw="<<s.raw_width<<"x"<<s.raw_height<<" active="<<s.width<<"x"<<s.height<<" margin="<<s.left_margin<<","<<s.top_margin<<" filters="<<r.imgdata.idata.filters<<"\ncam_mul=";
for(float v:c.cam_mul)std::cout<<v<<",";std::cout<<"\npre_mul=";for(float v:c.pre_mul)std::cout<<v<<",";std::cout<<"\nblack="<<c.black<<" maximum="<<c.maximum<<"\n";
e=r.unpack();if(e){std::cerr<<libraw_strerror(e);return 4;}if(!r.imgdata.rawdata.raw_image)return 5;
std::ofstream out(argv[2],std::ios::binary);for(size_t i=0,n=size_t(s.raw_width)*s.raw_height;i<n;i++){unsigned v=r.imgdata.rawdata.raw_image[i];char bytes[2]={char(v&255),char(v>>8)};out.write(bytes,2);}if(!out)return 6;std::cout<<"sensor_samples="<<size_t(s.raw_width)*s.raw_height<<"\n";
}
