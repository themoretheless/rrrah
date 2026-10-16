#include "pHash.h"
#include <vector>
#include <cassert>
#include <cmath>
#include <iostream>
#include <string>
static int bits(ulong64 a,ulong64 b){int n=0;for(int i=0;i<64;i++)n+=((a^b)>>i)&1;return n;}
static int oracle(const std::vector<ulong64>&a,const std::vector<ulong64>&b,int threshold){int best=0;for(unsigned mask=0;mask<(1u<<a.size());mask++){size_t j=0;int count=0;bool ok=true;for(size_t i=0;i<a.size();i++)if(mask&(1u<<i)){while(j<b.size()&&bits(a[i],b[j])>threshold)j++;if(j==b.size()){ok=false;break;}j++;count++;}if(ok&&count>best)best=count;}return best;}
static std::vector<std::vector<ulong64>> corpus(){std::vector<std::vector<ulong64>> out;for(int n=1;n<=4;n++){int count=1;for(int i=0;i<n;i++)count*=3;for(int k=0;k<count;k++){int v=k;std::vector<ulong64>s;for(int i=0;i<n;i++){ulong64 symbols[]={0,1,3};s.push_back(symbols[v%3]);v/=3;}out.push_back(s);}}return out;}
int main(int argc,char**argv){assert(argc==2);std::string mode=argv[1];
 if(mode=="oracle"){auto seq=corpus();int total=0;for(auto a:seq)for(auto b:seq)for(int threshold:{0,1,64}){double expected=oracle(a,b,threshold)/(double)std::min(a.size(),b.size());double actual=ph_dct_videohash_dist(a.data(),a.size(),b.data(),b.size(),threshold);assert(actual==expected);total++;}std::cout<<"{\"mode\":\"oracle\",\"sequences\":"<<seq.size()<<",\"comparisons\":"<<total<<"}\n";
 }else if(mode=="boundaries"){ulong64 a[]={1,2},b[]={7,1,3,2,5},other[]={~1ULL,~2ULL};assert(ph_dct_videohash_dist(a,2,b,5,0)==1);assert(ph_dct_videohash_dist(b,5,a,2,0)==1);assert(ph_dct_videohash_dist(a,2,other,2,64)==1);assert(ph_dct_videohash_dist(a,2,a,2,-1)==0);assert(std::isnan(ph_dct_videohash_dist(nullptr,0,nullptr,0,21)));assert(std::isnan(ph_dct_videohash_dist(a,2,nullptr,0,21)));std::cout<<"{\"mode\":\"boundaries\",\"subset_score\":1,\"reverse_subset_score\":1,\"threshold64_score\":1,\"negative_threshold_score\":0,\"empty_is_nan\":true,\"one_empty_is_nan\":true}\n";
 }else if(mode=="stack-limit"){std::vector<ulong64>a(4096,0),b(4096,1);std::cout<<ph_dct_videohash_dist(a.data(),a.size(),b.data(),b.size(),0)<<"\n";
 }else return 2;
}
