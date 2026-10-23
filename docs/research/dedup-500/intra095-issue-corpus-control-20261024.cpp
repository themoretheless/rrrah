#include <algorithm>
#include <fstream>
#include <random>
#include "intra095-20261024/deduplicator/bktree.hpp"
using namespace BK_Tree_Ns;
int main(int argc,char**argv){
 if(argc!=2)return 2;
 std::ifstream input(argv[1]);std::vector<std::pair<int64_t,int64_t>> values;
 int64_t id,hash;while(input>>id>>hash)values.push_back({id,hash});
 if(values.size()!=793)return 3;
 size_t checks=0;
 for(bool reverse:{false,true}){
  auto ordered=values;if(reverse)std::reverse(ordered.begin(),ordered.end());BK_Tree tree;
  for(auto item:ordered)tree.insert(item.second,item.first);
  std::vector<int64_t> queries={0,-1,INT64_MIN,INT64_MAX,-6076574518398440533LL};
  for(size_t i=0;i<values.size();i+=100)queries.push_back(values[i].second);
  for(auto query:queries)for(int radius=0;radius<=64;++radius){std::set<int64_t> expected;for(auto item:values)if(__builtin_popcountll(static_cast<uint64_t>(query^item.second))<=radius)expected.insert(item.first);if(tree.getWithinDistance(query,radius).first!=expected)return 4;++checks;}
  if(tree.getWithinDistance(-6076574518398440533LL,2).first.size()!=values.size())return 5;
 }
 std::cout<<"{\"rows\":"<<values.size()<<",\"brute_force_comparisons\":"<<checks<<",\"orders\":2,\"original_issue1_radius2_results\":793}\n";
}
