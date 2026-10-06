// Authored test harness; decompression is provided by the MIT PowerVR SDK.
#include "PVRTDecompress.h"
#include <algorithm>
#include <cstring>
#include <fstream>
#include <iostream>
#include <iterator>
#include <vector>
int main(int argc,char** argv) {
    if(argc!=5)return 2;
    const auto width=std::stoul(argv[2]),height=std::stoul(argv[3]),format=std::stoul(argv[4]);
    if(!width||!height||format>3)return 3;
    const uint32_t endian=1;if(*reinterpret_cast<const uint8_t*>(&endian)!=1)return 4;
    std::ifstream file(argv[1],std::ios::binary);
    std::vector<uint8_t> bytes((std::istreambuf_iterator<char>(file)),std::istreambuf_iterator<char>());
    const auto expected=std::max(width,format<2?16UL:8UL)*std::max(height,8UL)/(format<2?4:2);
    if(bytes.size()!=expected)return 5;
    std::vector<uint32_t> aligned(bytes.size()/4);std::memcpy(aligned.data(),bytes.data(),bytes.size());
    std::vector<uint8_t> out(width*height*4);
    pvr::PVRTDecompressPVRTC(aligned.data(),format<2,width,height,out.data());
    if(format%2==0)for(size_t i=3;i<out.size();i+=4)out[i]=255;
    std::cout.write(reinterpret_cast<const char*>(out.data()),out.size());return !std::cout;
}
