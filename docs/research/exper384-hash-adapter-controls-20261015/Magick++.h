#pragma once
#include <cstddef>
#include <cstdint>
#include <string>
#include <stdexcept>
namespace Magick {
enum ImageType {GrayscaleType};
enum StorageType {CharPixel};
struct Geometry {
 std::size_t width,height;
 Geometry(std::size_t w,std::size_t h):width(w),height(h){}
 void aspect(bool){}
};
class Image {
 std::string pattern;std::size_t w=0,h=0;
public:
 explicit Image(const std::string& name):pattern(name){}
 void resize(const Geometry& g){w=g.width;h=g.height;}
 void type(ImageType){}
 void write(int,int,std::size_t width,std::size_t height,const char* channels,StorageType,void* output){
  if(w!=width||h!=height||std::string(channels)!="R")throw std::runtime_error("adapter dimensions/channel mismatch");
  auto p=static_cast<std::uint8_t*>(output);
  for(std::size_t y=0;y<h;++y)for(std::size_t x=0;x<w;++x){
   p[y*w+x]=pattern=="x"?std::uint8_t(x*255/(w-1)):pattern=="y"?std::uint8_t(y*255/(h-1)):pattern=="white"?255:0;
  }
 }
};
}
