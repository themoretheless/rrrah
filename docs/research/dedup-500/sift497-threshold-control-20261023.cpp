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

int main(){
 DistanceMatrix chain;chain.n=3;chain.data={{0,1,2},{1,0,1},{2,1,0}};
 auto g=threshold_cluster(chain,1,2);assert(g.size()==1 && g[0].members.size()==3 && g[0].max_internal_distance==2);
 int checks=0;unsigned seed=1234567;
 for(int c=0;c<100;c++){
  DistanceMatrix dm;dm.n=c%17;dm.data.assign(dm.n,std::vector<int>(dm.n));std::vector<unsigned> hashes(dm.n);
  for(auto& h:hashes){seed=seed*1664525+1013904223;h=seed&255;}
  for(int i=0;i<dm.n;i++)for(int j=0;j<dm.n;j++)dm.data[i][j]=__builtin_popcount(hashes[i]^hashes[j]);
  int threshold=c%10-1;std::vector<std::vector<int>> expected;std::vector<bool> seen(dm.n);
  for(int i=0;i<dm.n;i++)if(!seen[i]){std::vector<int> members;std::queue<int> q;q.push(i);seen[i]=true;while(!q.empty()){int a=q.front();q.pop();members.push_back(a);for(int j=0;j<dm.n;j++)if(!seen[j]&&dm.data[a][j]<=threshold){seen[j]=true;q.push(j);}}std::sort(members.begin(),members.end());expected.push_back(members);}
  std::sort(expected.begin(),expected.end());
  for(int threads:{1,4}){auto groups=threshold_cluster(dm,threshold,threads);std::vector<std::vector<int>> actual;for(auto& group:groups){actual.push_back(group.members);int maxd=0;long long sum=0,count=0;for(size_t i=0;i<group.members.size();i++)for(size_t j=i+1;j<group.members.size();j++){int dist=dm.data[group.members[i]][group.members[j]];maxd=std::max(maxd,dist);sum+=dist;count++;}assert(group.max_internal_distance==maxd);assert(std::abs(group.avg_internal_distance-(count?double(sum)/count:0.))<1e-12);}std::sort(actual.begin(),actual.end());assert(actual==expected);checks++;}
 }
 std::cout<<checks<<" original threshold/group cases match BFS oracle; chain threshold1 has max-distance2\n";
}
