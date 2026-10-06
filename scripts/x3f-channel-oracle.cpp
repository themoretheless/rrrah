// Test-only LibRaw unpack oracle. No demosaic/WB/color processing.
#include <libraw/libraw.h>
#include <fstream>
#include <iostream>
int main(int argc,char **argv) {
    if(argc!=3)return 2;
    std::cerr<<"LibRaw "<<LibRaw::version()<<" X3FTOOLS="
        <<bool(LibRaw::capabilities()&LIBRAW_CAPS_X3FTOOLS)<<'\n';
    LibRaw raw;
    int status=raw.open_file(argv[1]);
    if(status==LIBRAW_SUCCESS)status=raw.unpack();
    if(status!=LIBRAW_SUCCESS){std::cerr<<libraw_strerror(status)<<'\n';return 1;}
    const auto &d=raw.imgdata;
    if(!d.rawdata.color3_image || d.sizes.raw_pitch<d.sizes.raw_width*6u){
        std::cerr<<"expected pitched three-channel sensor buffer\n";return 1;
    }
    std::ofstream output(argv[2],std::ios::binary);
    for(unsigned row=0;row<d.sizes.raw_height;++row){
        const auto *samples=reinterpret_cast<const unsigned short *>(
            reinterpret_cast<const char *>(d.rawdata.color3_image)+row*d.sizes.raw_pitch);
        for(unsigned sample=0;sample<d.sizes.raw_width*3u;++sample){
            char bytes[2]={char(samples[sample]&255),char(samples[sample]>>8)};
            output.write(bytes,2);
        }
    }
    if(!output)return 1;
    std::cout<<LibRaw::version()<<' '<<d.idata.make<<' '<<d.idata.model<<' '
        <<d.sizes.raw_width<<'x'<<d.sizes.raw_height<<" pitch="<<d.sizes.raw_pitch<<'\n';
}
