// CC0 synthetic ordinary EXR load controls, no zero-alpha emission.
#include <OpenEXR/ImfOutputFile.h>
#include <OpenEXR/ImfHeader.h>
#include <OpenEXR/ImfChannelList.h>
#include <OpenEXR/ImfFrameBuffer.h>
#include <OpenEXR/ImfStandardAttributes.h>
#include <vector>
#include <cassert>
#include <string>
namespace Ref=OPENEXR_IMF_NAMESPACE;
int main(int argc,char**argv){assert(argc==2);const int w=1024,h=1024;
 for(int c=0;c<2;c++){float a=c?0.5f:1.f;std::vector<float> p(w*h*4);for(int i=0;i<w*h;i++){p[i*4]=a*(1.f+(i%32)*.0625f);p[i*4+1]=a*(-.25f+(i%17)*.03125f);p[i*4+2]=a*.5f;p[i*4+3]=a;}
 Ref::Header header(w,h);header.compression()=Ref::NO_COMPRESSION;Ref::addChromaticities(header,Ref::Chromaticities());Ref::FrameBuffer f;int k=0;for(const char*name:{"R","G","B","A"}){header.channels().insert(name,Ref::Channel(Ref::FLOAT));f.insert(name,Ref::Slice(Ref::FLOAT,reinterpret_cast<char*>(p.data()+k++),16,w*16));}
 std::string path=std::string(argv[1])+(c?"/fractional.exr":"/opaque.exr");Ref::OutputFile out(path.c_str(),header);out.setFrameBuffer(f);out.writePixels(h);}}
