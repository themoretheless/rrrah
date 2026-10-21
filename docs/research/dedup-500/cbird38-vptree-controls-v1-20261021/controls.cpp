#include <cassert>
#include <cstdint>
#include <cstdio>
#include <random>
#include <set>
#define Q_ASSERT(x) assert(x)
#define qInfo(...) ((void)0)
#include "vptree.h"
struct Value {
 uint64_t hash; uint32_t id;
 static Value max(){return {UINT64_MAX,0};}
};
static int distance(Value a,Value b){return __builtin_popcountll(a.hash^b.hash);}
using Tree=VpTree<Value,int,distance>;
int main(){
 std::mt19937_64 rng(38);std::vector<Value> values;
 for(uint32_t i=1;i<=4096;i++)values.push_back({rng(),i});
 Tree tree;tree.create(values);
 size_t checks=0;
 for(int q=0;q<128;q++){
  Value target={q<64?values[q].hash:rng(),0};
  for(int threshold:{0,1,2,8,23,32,65}){
   std::vector<Value> out;std::vector<int> ds;tree.search(target,threshold,&out,&ds);
   std::set<uint32_t> actual,expected;
   for(size_t i=0;i<out.size();i++){actual.insert(out[i].id);assert(ds[i]==distance(target,out[i]));if(i)assert(ds[i-1]<=ds[i]);}
   for(auto v:values)if(distance(target,v)<threshold)expected.insert(v.id);
   assert(actual==expected);assert(actual.size()==out.size());checks++;
  }
 }
 printf("{\"name\":\"seeded-active-vptree-matches-exhaustive-hamming-reference\",\"passed\":true,\"queries_times_thresholds\":%zu}\n",checks);
 std::vector<Value> duplicates;for(uint32_t i=1;i<=60;i++)duplicates.push_back({123,i});
 tree.create(duplicates);std::vector<Value> out;std::vector<int> ds;
 tree.search({123,0},1,&out,&ds);assert(out.size()==60);
 tree.search({123,0},0,&out,&ds);assert(out.empty());
 printf("{\"name\":\"sixty-identical-values-retained-and-threshold-is-exclusive\",\"passed\":true}\n");
 std::vector<Value> tombstones={{0,0},{1,1}};tree.create(tombstones);tree.search({1,0},2,&out,&ds);
 assert(out.size()==2);assert(out[1].id==0);
 printf("{\"name\":\"tree-does-not-filter-zero-id-zero-hash-tombstone\",\"passed\":true}\n");
}
