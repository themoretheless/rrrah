#include "cluster.hpp"
#include <cassert>
#include <algorithm>
#include <iostream>
#include <queue>
int HashResult::hamming(const HashResult& a, const HashResult& b) {
    int dist = 0;
    int len = (int)std::min(a.bits.size(), b.bits.size());
    for (int i = 0; i < len; i++) {
        dist += __builtin_popcount(a.bits[i] ^ b.bits[i]);
    }
    return dist;
}

#include "/Users/themoretheless/Documents/Sources/rrrah/docs/research/dedup-500/evidence/Mahoskian--sift/02-distance.cpp"
#include "src/cluster/thresh.cpp"

#include "/Users/themoretheless/Documents/Sources/rrrah/docs/research/dedup-500/evidence/Mahoskian--sift/03-hdbscan.cpp"

int main(){
 int checks=0;unsigned seed=91;
 for(int c=0;c<40;c++){
  int n=c%2?32:8; DistanceMatrix m;m.n=n;m.data.assign(n,std::vector<int>(n));std::vector<unsigned> values(n);
  for(int i=0;i<n;i++){seed=seed*1664525+1013904223;values[i]=c<4?0:(c<12?(i<n/2?0:255):(seed&255));}
  for(int i=0;i<n;i++)for(int j=0;j<n;j++)m.data[i][j]=__builtin_popcount(values[i]^values[j]);
  for(int minimum:{2,3,5}){
   auto a=hdbscan_cluster(m,minimum,1),b=hdbscan_cluster(m,minimum,4);
   assert(a.membership.size()==size_t(n)&&b.membership.size()==size_t(n));assert(a.groups.size()==b.groups.size());
   for(int i=0;i<n;i++){auto x=a.membership[i],y=b.membership[i];assert(x.file==i&&x.group==y.group&&x.confidence==y.confidence);assert(std::isfinite(x.confidence)&&x.confidence>=0&&x.confidence<=1);}
   for(size_t i=0;i<a.groups.size();i++)assert(a.groups[i].members==b.groups[i].members);
   if(c==0&&minimum==2)std::cout<<"all-identical n8 groups="<<a.groups.size()<<"\n";
   checks++;
  }
 }
 std::cout<<checks<<" thread parity and finite confidence cases passed\n";
}
