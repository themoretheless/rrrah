#include "intra095-20261024/deduplicator/bktree.hpp"
using namespace BK_Tree_Ns;
int main(){std::cerr<<"main entered"<<std::endl;
 BK_Tree tree;tree.insert(0,0);tree.insert(-1,1);tree.insert(1,2);tree.insert(1,3);
 std::cerr<<"before root removal"<<std::endl;tree.remove(0,0);std::cerr<<"after root removal"<<std::endl;auto afterroot=tree.get_all();std::cerr<<"after snapshot"<<std::endl;tree.remove(1,2);std::cerr<<"after second removal"<<std::endl;
 auto exact=tree.getWithinDistance(1,0).first;
 bool ghost=exact.count(2)!=0;
 std::cout<<"{\"assertions_enabled\":false,\"items_after_root_removal\":"<<afterroot.size()<<",\"expected_items_after_root_removal\":3,\"deleted_id_still_found\":"<<(ghost?"true":"false")<<"}\n";
 return afterroot.size()==6 && !ghost ? 0 : 2;
}
