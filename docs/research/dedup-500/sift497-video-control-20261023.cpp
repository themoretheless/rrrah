#include "video_hash.hpp"
#include <cassert>
#include <algorithm>
#include <chrono>
#include <iostream>
#include <stdexcept>
#include "src/hash/video_hash.cpp"
struct Frames:io::FrameSource {
 mutable std::filesystem::path directory;
 std::vector<int> values;
 std::string strategy_name()const override{return "controlled";}
 std::vector<std::filesystem::path> extract(const std::filesystem::path&,const std::filesystem::path& out)const override{
 directory=out;std::vector<std::filesystem::path> r;for(int v:values)r.emplace_back(std::to_string(v));return r;
 }
};
int main(){
 auto video=std::filesystem::path("sift-review-control-"+std::to_string(std::chrono::steady_clock::now().time_since_epoch().count()));
 auto fn=[](const std::string& p,int size){int v=std::stoi(p);if(v<0)return HashResult{};HashResult h;h.size=size;h.bits={static_cast<unsigned char>(v)};return h;};
 unsigned seed=73;int checks=0;
 for(int c=0;c<128;c++){
  Frames f;for(int i=0;i<c%11;i++){seed=seed*1664525+1013904223;f.values.push_back(i%4==0?-1:int(seed&240));}
  auto h=hash_video(video,f,fn,2);int valid=0;unsigned expected=0;for(int v:f.values)if(v>=0)valid++;
  if(valid){for(int bit=4;bit<8;bit++){int count=0;for(int v:f.values)if(v>=0&&(v&(1<<bit)))count++;if(count*2>valid)expected|=1<<bit;}assert(h.bits.size()==1&&h.bits[0]==expected);}else assert(h.bits.empty());
  assert(!std::filesystem::exists(f.directory));std::reverse(f.values.begin(),f.values.end());auto reversed=hash_video(video,f,fn,2);assert(reversed.bits==h.bits);checks++;
 }
 Frames f;f.values={240,0};assert(hash_video(video,f,fn,2).bits[0]==0);
 bool caught=false;try{hash_video(video,f,[](const std::string&,int)->HashResult{throw std::runtime_error("controlled failure");},2);}catch(const std::runtime_error&){caught=true;}
 assert(caught&&std::filesystem::exists(f.directory));std::filesystem::remove_all(f.directory);
 std::cout<<checks<<" independent vote-oracle and reversal cases passed; tie bits zero; exception leaves own temporary directory\n";
}
