#include <bit>
#include <cassert>
#include <cstdint>
#include <iostream>
#include <map>
#include <string>
#include <utility>
#include <vector>
using QString=std::string;
using ulong64=std::uint64_t;
template<class A,class B>using QPair=std::pair<A,B>;
template<class T>struct QList {
 std::vector<T> rows;
 int count()const{return static_cast<int>(rows.size());}
 const T& at(int i)const{return rows.at(i);}
 QList& operator<<(const T& v){rows.push_back(v);return *this;}
};
template<class K,class V>struct QHash {
 std::vector<std::pair<K,V>> rows;
 void insertMulti(const K&k,const V&v){rows.emplace_back(k,v);}
 int count()const{return static_cast<int>(rows.size());}
 bool contains(const K&k)const{for(const auto&p:rows)if(p.first==k)return true;return false;}
 V& operator[](const K&k){for(auto&p:rows)if(p.first==k)return p.second;rows.emplace_back(k,V{});return rows.back().second;}
 QList<K> keys()const{QList<K> r;for(const auto&p:rows)r<<p.first;return r;}
 QList<V> values()const{QList<V> r;for(const auto&p:rows)r<<p.second;return r;}
};
static int HAMMINGDIST=16;
int ph_hamming_distance(ulong64 a,ulong64 b){return std::popcount(a^b);}
#include "original-core.cpp"
int main(){
 int checks=0;auto check=[&](bool value){assert(value);++checks;};
 QHash<ulong64,QPair<int,QString>> boundary;
 boundary.insertMulti((1ULL<<15)-1,{0,"d15"});boundary.insertMulti((1ULL<<16)-1,{0,"d16"});boundary.insertMulti(0,{0,"d0"});
 check(closestHashes(boundary,0).count()==2);
 HAMMINGDIST=0;check(closestHashes(boundary,0).count()==0);
 HAMMINGDIST=1;check(closestHashes(boundary,0).count()==1);
 HAMMINGDIST=65;check(closestHashes(boundary,0).count()==3);HAMMINGDIST=16;
 QHash<ulong64,QPair<int,QString>> repeated;
 for(int i=0;i<5;++i)repeated.insertMulti(0,{i,"old"});
 QHash<ulong64,int> one;one.insertMulti(0,0);
 auto matches=fileMatches("new",one,repeated);check(matches["old"]==5);check(matches["old"]>one.count());
 repeated.rows.clear();for(int i=0;i<5;++i)repeated.insertMulti(0,{i,"old"});
 QHash<ulong64,int> five;for(int i=0;i<5;++i)five.insertMulti(0,i);
 matches=fileMatches("new",five,repeated);check(matches["old"]==25);check(matches["old"]!=five.count());
 QHash<ulong64,QPair<int,QString>> self;for(int i=0;i<5;++i)self.insertMulti(0,{i,"new"});
 check(fileMatches("new",five,self).count()==0);
 std::uint64_t seed=173;
 auto random=[&](){seed^=seed<<13;seed^=seed>>7;seed^=seed<<17;return seed;};
 for(int trial=0;trial<200;++trial){
  QHash<ulong64,QPair<int,QString>> known;QHash<ulong64,int> query;
  for(int i=0;i<20;++i)known.insertMulti(i%3==0?0:random(),{i,i%4==0?"new":std::to_string(i%5)});
  for(int i=0;i<5;++i)query.insertMulti(i%2==0?0:random(),i);
  std::map<QString,int> expected;
  for(const auto&q:query.rows)for(const auto&k:known.rows)if(k.second.second!="new"&&std::popcount(q.first^k.first)<16)++expected[k.second.second];
  auto actual=fileMatches("new",query,known);std::map<QString,int> observed;for(const auto&p:actual.rows)observed.emplace(p.first,p.second);
  check(observed==expected);check(known.count()==25);
 }
 std::cout<<"CHECKS="<<checks<<" ONE_QUERY_FIVE_MATCHES=5 FIVE_QUERY_MATCHES=25\n";
}
