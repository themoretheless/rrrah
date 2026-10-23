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

#include "src/cluster/hierarchical.cpp"

int main(){
 DistanceMatrix dm;dm.n=3;dm.data={{0,1,2},{1,0,1},{2,1,0}};
 auto single=hierarchical_cluster(dm,1,"single",1);auto complete=hierarchical_cluster(dm,1,"complete",1);auto average=hierarchical_cluster(dm,1,"average",1);
 assert(single.groups.size()==1);assert(complete.groups.size()==2);assert(average.groups.size()==1);
 assert(average_linkage({0,1},{2},dm)==1); // independent exact mean=(2+1)/2=1.5
 assert(hierarchical_cluster(dm,1,"typo",1).groups.size()==complete.groups.size());
 int checks=0;unsigned seed=12345;
 for(int c=0;c<10;c++){
  DistanceMatrix m;m.n=32;m.data.assign(32,std::vector<int>(32));std::vector<unsigned> values(32);for(auto& x:values){seed=seed*1664525+1013904223;x=seed&255;}
  for(int i=0;i<32;i++)for(int j=0;j<32;j++)m.data[i][j]=__builtin_popcount(values[i]^values[j]);
  for(const char* method:{"single","complete","average"}){
   auto a=hierarchical_cluster(m,c%9,method,1);auto b=hierarchical_cluster(m,c%9,method,4);
   assert(a.groups.size()==b.groups.size());for(size_t i=0;i<a.groups.size();i++)assert(a.groups[i].members==b.groups[i].members);
   assert(a.dendrogram.size()==b.dendrogram.size());for(size_t i=0;i<a.dendrogram.size();i++){auto x=a.dendrogram[i],y=b.dendrogram[i];assert(x.step==y.step&&x.merged_a==y.merged_a&&x.merged_b==y.merged_b&&x.distance==y.distance);}checks++;
  }
 }
 std::cout<<"chain cut1 group counts single1 complete2 average1; average1.5 truncated1; "<<checks<<" thread1/4 group+dendrogram parity cases passed\n";
}
