"""Execute original grouping bodies with synthetic Qt-container/Media shims."""
from pathlib import Path
import subprocess,sys
root=Path(sys.argv[1]);source=(root/'src/media.cpp').read_text();header=(root/'src/media.h').read_text()
methods=source[source.index('bool Media::groupCompareByContents('):source.index('void Media::sortGroupList(')]
operator=header[header.index('  bool operator<(const Media& other) const {'):header.index('  /// Compare by path')]
shim=r'''
#include <algorithm>
#include <cassert>
#include <cstdio>
#include <string>
#include <vector>
#include <unordered_set>
using QString=std::string;
template<class T> struct QVector:std::vector<T>{using std::vector<T>::vector;int count()const{return this->size();}bool isEmpty()const{return this->empty();}void append(const T&v){this->push_back(v);}bool contains(const T&v)const{return std::find(this->begin(),this->end(),v)!=this->end();}};
template<class T>struct QSet:std::unordered_set<T>{bool contains(const T&v)const{return this->find(v)!=this->end();}};
class Media;using MediaGroup=QVector<Media>;using MediaGroupList=QVector<MediaGroup>;
class Media{public:QString p;int _score=0,_matchFlags=0;enum{MatchExact=1};Media(QString p,int score=0):p(p),_score(score){}const QString&path()const{return p;}bool operator==(const Media&o)const{return path()==o.path();}
'''
shim+=operator+r'''
static bool groupCompareByContents(const MediaGroup&,const MediaGroup&);static int indexInGroupByPath(MediaGroup&,const QString&);static void mergeGroupList(MediaGroupList&);static void expandGroupList(MediaGroupList&);};
'''
tests=r'''
int main(){
Media a("a",0),b("b",1),c("c",2);
MediaGroupList chain={{a,b},{b,c}};Media::mergeGroupList(chain);assert(chain.size()==1&&chain[0].size()==3);
printf("{\"name\":\"anchor-chain-merges-without-testing-a-c\",\"passed\":true}\n");
MediaGroupList overlap={{a,b},{c,b}};Media::mergeGroupList(overlap);assert(overlap.size()==2);
printf("{\"name\":\"shared-nonanchor-does-not-merge\",\"passed\":true}\n");
assert(Media::groupCompareByContents({a,b},{a,a}));assert(!Media::groupCompareByContents({a,a},{a,b}));
printf("{\"name\":\"group-comparison-is-asymmetric-with-duplicate-paths\",\"passed\":true}\n");
Media exact("e");exact._matchFlags=Media::MatchExact;assert(exact<exact);
printf("{\"name\":\"exact-flag-comparator-is-not-irreflexive\",\"passed\":true}\n");
MediaGroupList star={{a,b,c}};Media::expandGroupList(star);assert(star.size()==2&&star[0]==MediaGroup({a,b})&&star[1]==MediaGroup({a,c}));
printf("{\"name\":\"expansion-retains-anchor-edges-only\",\"passed\":true}\n");}
'''
cpp=root/'group-controls.cpp';binary=root/'group-controls';cpp.write_text(shim+methods+tests)
subprocess.run(['clang++','-std=c++17','-O2',str(cpp),'-o',str(binary)],check=True)
subprocess.run([str(binary.resolve())],check=True)
