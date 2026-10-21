from pathlib import Path
import sys,subprocess
root=Path(sys.argv[1]);source=(root/'src/dctfeaturesindex.cpp').read_text();method=source[source.index('QVector<Index::Match> DctFeaturesIndex::find('):]
shim=r'''
#include <algorithm>
#include <cassert>
#include <cstdint>
#include <cstdio>
#include <map>
#include <vector>
#include <string>
#define Q_ASSERT(x) assert(x)
#define qWarning() Sink{}
#define qInfo(...) ((void)0)
struct Sink{template<class T>Sink&operator<<(const T&){return *this;}};
uint64_t nanoTime(){return 1000;}int hamm64(uint64_t a,uint64_t b){return __builtin_popcountll(a^b);}using dcthash_t=uint64_t;using KeyPointHashList=std::vector<uint64_t>;
template<class T>struct QVector:std::vector<T>{using std::vector<T>::vector;void append(const T&x){this->push_back(x);}};
template<class K,class V>struct QMap:std::map<K,V>{bool contains(K x)const{return this->find(x)!=this->end();}int count()const{return this->size();}std::vector<K>keys()const{std::vector<K>r;for(auto&x:*this)r.push_back(x.first);return r;}};
struct Media{int id()const{return 100;}std::string path()const{return "synthetic";}KeyPointHashList keyPointHashes()const{return {0};}};
struct SearchParams{int dctThresh=1;bool verbose=false;};struct Index{struct Match{uint32_t mediaId;int score;};};
struct HammingTree{struct Value{uint32_t index;uint64_t hash=0;};struct Match{Value value;int distance=0;};std::vector<Match>given;void search(uint64_t,int,std::vector<Match>&out){out=given;}void findIndex(int,KeyPointHashList&){}size_t size()const{return given.size();}};
struct DctFeaturesIndex{HammingTree*_tree;QVector<Index::Match>find(const Media&,const SearchParams&);};
'''
tests=r'''
int main(){HammingTree tree;DctFeaturesIndex index{&tree};for(uint32_t i=1;i<=60;i++)tree.given.push_back({{i,0},0});auto r=index.find(Media{},SearchParams{});assert(r.size()==10);puts("{\"name\":\"sixty-equal-feature-candidates-truncated-to-ten\",\"passed\":true}");tree.given.clear();for(int i=0;i<10;i++)tree.given.push_back({{0,0},0});tree.given.push_back({{1,0},0});r=index.find(Media{},SearchParams{});assert(r.empty());puts("{\"name\":\"ten-deleted-entries-consume-cap-before-valid-eleventh\",\"passed\":true}");tree.given.clear();for(int i=0;i<10;i++)tree.given.push_back({{1,0},0});tree.given.push_back({{2,0},0});r=index.find(Media{},SearchParams{});assert(r.size()==1&&r[0].mediaId==1);puts("{\"name\":\"same-file-feature-multiplicity-can-exclude-other-file\",\"passed\":true}");}
'''
cpp=root/'feature-controls.cpp';binary=root/'feature-controls';cpp.write_text(shim+method+tests);subprocess.run(['clang++','-std=c++17','-O2',str(cpp),'-o',str(binary)],check=True);subprocess.run([str(binary.resolve())],check=True)
