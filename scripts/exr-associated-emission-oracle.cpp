// CC0 authored two-pixel linear Rec.709 associated-alpha HDR control.
// OpenEXR is a reference producer/reader only; Rrrah runtime remains Rust.
#include <OpenEXR/ImfInputFile.h>
#include <OpenEXR/ImfOutputFile.h>
#include <OpenEXR/ImfFrameBuffer.h>
#include <OpenEXR/ImfHeader.h>
#include <OpenEXR/ImfChannelList.h>
#include <OpenEXR/ImfStandardAttributes.h>
#include <array>
#include <cassert>
#include <cstdint>
#include <cstring>
#include <fstream>
#include <iostream>
#include <string>
namespace ReferenceExr=OPENEXR_IMF_NAMESPACE;
static ReferenceExr::FrameBuffer frame(std::array<float,8>& values) {
    ReferenceExr::FrameBuffer result;const char* names[]={"R","G","B","A"};
    for(int c=0;c<4;c++)result.insert(names[c],ReferenceExr::Slice(ReferenceExr::FLOAT,reinterpret_cast<char*>(values.data()+c),16,32));
    return result;
}
static void little(const std::string& path,const float* values,int count) {
    std::ofstream out(path,std::ios::binary);assert(out);
    for(int i=0;i<count;i++){uint32_t bits;std::memcpy(&bits,values+i,4);for(int b=0;b<4;b++)out.put(char(bits>>(b*8)));}
    assert(out);
}
int main(int argc,char**argv) {
    assert(argc==2);std::string path=argv[1];
    std::array<float,8> authored={2,0.5f,0.25f,0,-0.25f,4,0.5f,0.5f};
    ReferenceExr::Header header(2,1);header.compression()=ReferenceExr::ZIP_COMPRESSION;
    for(const char* name:{"R","G","B","A"})header.channels().insert(name,ReferenceExr::Channel(ReferenceExr::FLOAT));
    ReferenceExr::addChromaticities(header,ReferenceExr::Chromaticities());
    {ReferenceExr::OutputFile output(path.c_str(),header);output.setFrameBuffer(frame(authored));output.writePixels(1);}
    std::array<float,8> decoded{};ReferenceExr::InputFile input(path.c_str());
    assert(input.header().channels().findChannel("R")->type==ReferenceExr::FLOAT);
    input.setFrameBuffer(frame(decoded));input.readPixels(0,0);
    assert(std::memcmp(authored.data(),decoded.data(),32)==0);
    float background[3]={0.25f,0.5f,0.75f};std::array<float,6> over{};
    for(int p=0;p<2;p++)for(int c=0;c<3;c++)over[p*3+c]=decoded[p*4+c]+(1-decoded[p*4+3])*background[c];
    std::array<float,6> expected={2.25f,1,1,-0.125f,4.25f,0.875f};assert(over==expected);
    little(path+".native-rgba32le",decoded.data(),8);little(path+".over-rgb32le",over.data(),6);
    std::cout<<"{\"width\":2,\"height\":1,\"source_rgb\":\"linear Rec709/D65\",\"alpha\":\"associated\",\"native_f32_bits_exact\":true,\"background\":[0.25,0.5,0.75],\"over_rgb\":[[2.25,1,1],[-0.125,4.25,0.875]]}\n";
}
