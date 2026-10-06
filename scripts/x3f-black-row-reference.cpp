// Test-only reference arithmetic; input is the independent SD10 unpack/CAMF oracle.
#include <array>
#include <cmath>
#include <cstdint>
#include <cstring>
#include <fstream>
#include <iterator>
#include <vector>
static std::vector<unsigned char> read(const char *p) {
    std::ifstream f(p,std::ios::binary);
    return {std::istreambuf_iterator<char>(f),{}};
}
static uint32_t u32(const std::vector<unsigned char>& b,size_t o) {
    return b.at(o)|(uint32_t(b.at(o+1))<<8)|(uint32_t(b.at(o+2))<<16)|(uint32_t(b.at(o+3))<<24);
}
static float f32(const std::vector<unsigned char>& b,size_t o) {
    uint32_t bits=u32(b,o);float value;std::memcpy(&value,&bits,4);return value;
}
int main(int argc,char **argv) {
    if(argc!=4)return 2;
    auto sensor=read(argv[1]),camf=read(argv[2]);
    constexpr unsigned width=2304,height=1531;
    if(sensor.size()!=size_t(width)*height*6)return 1;
    size_t drift=0;
    for(size_t p=0;p<camf.size();) {
        auto size=u32(camf,p+8),name=u32(camf,p+12),value=u32(camf,p+16);
        if(!size||p+size>camf.size())return 1;
        if(std::strcmp(reinterpret_cast<const char*>(camf.data()+p+name),"DarkDrift")==0)
            drift=p+u32(camf,p+value+8);
        p+=size;
    }
    if(!drift)return 1;
    auto pixel=[&](unsigned row,unsigned col,unsigned channel) {
        size_t p=(size_t(row)*width+col)*6+channel*2;
        uint16_t bits=uint16_t(sensor.at(p))|(uint16_t(sensor.at(p+1))<<8);
        return float(int16_t(bits));
    };
    auto average=[&](unsigned row,unsigned first,unsigned last,unsigned c) {
        float sum=0,lo=INFINITY,hi=-INFINITY;
        for(unsigned x=first;x<=last;++x) {
            float current=pixel(row,x,c);
            float v=current+(current-pixel(row,x-1,c))*0.8f;
            sum+=v;lo=std::fmin(lo,v);hi=std::fmax(hi,v);
        }
        return (sum-lo-hi)/float(last-first-1);
    };
    std::ofstream out(argv[3],std::ios::binary);
    for(unsigned row=0;row<height;++row)for(unsigned c=0;c<3;++c) {
        float coefficients[2];
        for(unsigned k=0;k<2;++k) {
            float top=f32(camf,drift+(c*2+k)*4),bottom=f32(camf,drift+(6+c*2+k)*4);
            coefficients[k]=top+double(row)/double(height-1)*(bottom-top);
        }
        float value=(average(row,6,14,c)+3.f*average(row,2292,2300,c)-coefficients[0])/4.f-coefficients[1];
        uint32_t bits;std::memcpy(&bits,&value,4);
        char bytes[4]={char(bits),char(bits>>8),char(bits>>16),char(bits>>24)};out.write(bytes,4);
    }
    return out?0:1;
}
