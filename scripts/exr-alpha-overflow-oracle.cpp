// CC0 authored FLOAT EXR boundary samples; OpenEXR is a test oracle only.
#include <OpenEXR/ImfOutputFile.h>
#include <OpenEXR/ImfInputFile.h>
#include <OpenEXR/ImfHeader.h>
#include <OpenEXR/ImfChannelList.h>
#include <OpenEXR/ImfFrameBuffer.h>
#include <OpenEXR/ImfStandardAttributes.h>
#include <array>
#include <cassert>
#include <cmath>
#include <cstdint>
#include <cstring>
#include <fstream>
#include <limits>
namespace E=OPENEXR_IMF_NAMESPACE;
int main(int argc,char**argv){
 assert(argc==2);
 const float max=std::numeric_limits<float>::max();
 // Overflow occurs only at the last pixel; earlier pixels must stay associated too.
 std::array<float,16> values={1,-2,3,1, .25f,-.5f,1,.5f, -0.f,0,0,0, max,-max,1,std::numeric_limits<float>::min()};
 E::Header h(4,1);h.compression()=E::ZIP_COMPRESSION;E::addChromaticities(h,E::Chromaticities());
 const char* names[]={"R","G","B","A"};for(auto name:names)h.channels().insert(name,E::Channel(E::FLOAT));
 auto frame=[&](float* p){E::FrameBuffer f;for(int c=0;c<4;c++)f.insert(names[c],E::Slice(E::FLOAT,reinterpret_cast<char*>(p+c),16,64));return f;};
 {E::OutputFile out(argv[1],h);out.setFrameBuffer(frame(values.data()));out.writePixels(1);}
 std::array<float,16> read{};E::InputFile in(argv[1]);in.setFrameBuffer(frame(read.data()));in.readPixels(0,0);
 assert(std::memcmp(read.data(),values.data(),sizeof(values))==0);
 volatile float alpha=read[15]; assert(std::isinf(read[12]/alpha));assert(std::isinf(read[13]/alpha));
 std::ofstream out(std::string(argv[1])+".native-rgba32le",std::ios::binary);
 for(float v:read){uint32_t bits;std::memcpy(&bits,&v,4);for(int b=0;b<4;b++)out.put(char(bits>>(8*b)));}assert(out);
}
