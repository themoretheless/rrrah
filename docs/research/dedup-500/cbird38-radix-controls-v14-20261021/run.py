from pathlib import Path
import subprocess,sys
root=Path(sys.argv[1]);s=(root/'src/tree/radix.h').read_text().replace('#include "../hamm.h"','')
shim=r'''
#include <algorithm>
#include <cmath>
#include <climits>
#include <cstdint>
#include <vector>
#include <cstdio>
#include <cassert>
using uint=unsigned;using dcthash_t=uint64_t;
#define Q_DISABLE_COPY_MOVE(x)
#define Q_LIKELY(x) (x)
#define Q_UNLIKELY(x) (x)
#define qWarning(...) ((void)0)
int hamm64(uint64_t a,uint64_t b){return __builtin_popcountll(a^b);}
'''
tests=r'''
int main(){using T=RadixMap_t<>;T tree(1);tree.insert({{1,2},{2,4}});std::vector<T::Match>r;tree.search(0,2,r);assert(r.size()==1&&r[0].value.index==2);puts("{\"name\":\"distance-one-cross-bucket-candidate-is-missed\",\"passed\":true}");T full(0);full.insert({{1,2},{2,4}});r.clear();full.search(0,2,r);assert(r.size()==2);puts("{\"name\":\"zero-radix-scalar-control-retains-both\",\"passed\":true}");uint64_t q[8]={0,2,0,0,0,0,0,0};std::vector<T::Match>out[8];tree.search(q,1,out);assert(out[1].empty());r.clear();tree.search(2,1,r);assert(r.size()==1&&r[0].value.index==1);puts("{\"name\":\"mixed-bucket-batch-searches-first-query-bucket-only\",\"passed\":true}");}
'''
p=root/'radix-controls.cpp';p.write_text(shim+s+tests);binary=root/'radix-controls';subprocess.run(['clang++','-std=c++20','-O2',str(p),'-o',str(binary)],check=True);subprocess.run([str(binary.resolve())],check=True)
