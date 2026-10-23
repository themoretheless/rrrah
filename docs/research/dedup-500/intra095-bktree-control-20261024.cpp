#include <algorithm>
#include <limits>
#include <random>
#include "../intra095-20261024/deduplicator/bktree.hpp"
using namespace BK_Tree_Ns;
int main(int argc,char**) {
 BK_Tree tree;
 tree.insert(0,0);tree.insert(-1,1);tree.insert(1,2);tree.insert(1,3);
 if(argc>1){tree.remove(0,0);std::cout<<"root removal survived\n";return 0;}
 std::vector<std::pair<int64_t,int64_t>> values={{0,0},{-1,1},{1,2},{1,3}};
 std::mt19937_64 rng(95);
 for(int i=4;i<132;++i){int64_t h=static_cast<int64_t>(rng());values.push_back({h,i});tree.insert(h,i);}
 auto before=tree.get_all();auto missing=tree.remove(1,999999);auto after=tree.get_all();
 std::sort(before.begin(),before.end());std::sort(after.begin(),after.end());if(before!=after)return 11;
 int checks=0;
 auto check=[&](){for(int q=0;q<64;++q){int64_t hash=values[q%values.size()].first;for(int radius=0;radius<=64;++radius){std::set<int64_t> expected;for(auto item:values)if(__builtin_popcountll(static_cast<uint64_t>(hash^item.first))<=radius)expected.insert(item.second);if(tree.getWithinDistance(hash,radius).first!=expected)return false;++checks;}}return true;};
 if(!check())return 12;
 for(int id=4;id<36;++id){tree.remove(values[4].first,values[4].second);values.erase(values.begin()+4);}
 if(!check())return 13;
 std::cout<<"{\"brute_force_comparisons\":"<<checks<<",\"missing_id_reported_deleted\":"<<missing[0]<<",\"missing_id_changed_contents\":false,\"remaining_items\":"<<tree.get_all().size()<<",\"hamming_zero_allones\":"<<f_hamming(0,-1)<<"}\n";
}
