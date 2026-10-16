#include "pHash.h"
#include <cassert>
#include <iostream>
#include <fstream>
#include <iomanip>
#include <sstream>
#include <string>
extern CImg<float>* GetMHKernel(float,float);
static std::string pattern(const std::string& dir,const std::string& name,unsigned seed) {
 std::string path=dir+"/"+name+".ppm";std::ofstream f(path,std::ios::binary);f<<"P6\n64 64\n255\n";
 for(int i=0;i<64*64*3;i++){seed^=seed<<13;seed^=seed>>17;seed^=seed<<5;f.put((char)(seed&255));}return path;
}
static std::string mh(const std::string& p,float alpha){int n=0;auto x=ph_mh_imagehash(p.c_str(),n,alpha,1);assert(x&&n==72);std::ostringstream s;for(int i=0;i<n;i++)s<<std::hex<<std::setw(2)<<std::setfill('0')<<(unsigned)x[i];free(x);return s.str();}
int main(int argc,char** argv){assert(argc==3);std::string mode=argv[1],dir=argv[2];
 if(mode=="hamming") {int count=0;for(int a=0;a<256;a++)for(int b=0;b<256;b++){uint8_t x=a,y=b;int oracle=0;for(int j=0;j<8;j++)oracle+=((a^b)>>j)&1;assert(ph_hammingdistance2(&x,1,&y,1)==oracle/8.0);count++;}
  for(int bit=0;bit<64;bit++)assert(ph_hamming_distance(0,((ulong64)1)<<bit)==1);
  uint8_t a=0,b=1;assert(ph_hammingdistance2(&a,1,&b,0)==-1);assert(ph_hammingdistance2(NULL,1,&b,1)==-1);assert(ph_hammingdistance2(&a,0,&b,0)==-1);
  std::cout<<"{\"mode\":\"hamming\",\"byte_pairs\":"<<count<<",\"bit_positions\":64,\"invalid_cases\":3}\n";
 } else if(mode=="mh2first"||mode=="mh3first") {auto p=pattern(dir,"mh",12345);float first=mode=="mh2first"?2:3,second=first==2?3:2;auto a=mh(p,first),b=mh(p,second);auto k=GetMHKernel(second,1);
  assert(a==b);std::cout<<"{\"mode\":\""<<mode<<"\",\"first\":\""<<a<<"\",\"second\":\""<<b<<"\",\"kernel_width\":"<<k->width()<<"}\n";
 } else if(mode=="dct") {std::cout<<"{\"mode\":\"dct\",\"hashes\":[";for(int i=0;i<32;i++){auto p=pattern(dir,"dct-"+std::to_string(i),12345+i);ulong64 hash=0;assert(ph_dct_imagehash(p.c_str(),hash)==0);assert((hash&1)==0);if(i)std::cout<<",";std::cout<<hash;}std::cout<<"]}\n";
 } else if(mode=="text-positive") {TxtHashPoint a[2]={{1,0},{2,1}},b[2]={{1,0},{3,1}};int n=0;auto x=ph_compare_text_hashes(a,2,b,2,&n);assert(x&&n==1&&x[0].length==1);free(x);std::cout<<"{\"mode\":\"text-positive\",\"matches\":1}\n";
 } else if(mode=="text-overflow") {TxtHashPoint a[2]={{1,0},{1,1}},b[2]={{1,0},{1,1}};int n=0;auto x=ph_compare_text_hashes(a,2,b,2,&n);std::cout<<n<<"\n";free(x);
 } else return 2;
}
