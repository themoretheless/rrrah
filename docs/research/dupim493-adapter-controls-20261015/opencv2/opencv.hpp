#pragma once
#include <string>
#include <vector>
namespace cv {
constexpr int IMREAD_UNCHANGED=-1,IMWRITE_JPEG_QUALITY=1;
struct Mat {std::string source;bool invalid;bool empty()const{return invalid;}};
Mat imread(const std::string&,int);
bool imwrite(const std::string&,const Mat&,const std::vector<int>&);
}
