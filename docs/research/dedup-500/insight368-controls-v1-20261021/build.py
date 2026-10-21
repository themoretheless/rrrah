from pathlib import Path
R=Path(__file__).resolve().parents[4];D=Path(__file__).resolve().parent
s=(R/'docs/research/dedup-500/evidence/ramenhut--insight/02-vnInsight.cpp').read_text()
start=s.index('FLOAT32 vnCompareImages64(');end=s.index('FLOAT32 vnCompareImages(',start)
fn=s[start:end]
license_text=(R/'docs/research/dedup-500/evidence/ramenhut--insight/03-vnInsight.h').read_text().split('// Description:')[0]
(D/'extracted.cpp').write_text(license_text+'''// Copyright (c) 2009-2014 Joe Bertolami. Original function under BSD-2-Clause.
// License text retained in source-review evidence and original vnInsight.h.
#include <cstdint>
#include <cassert>
#include <iostream>
using UINT64=uint64_t;using UINT32=uint32_t;using FLOAT32=float;
#define CONST const
#define VN_INSIGHT_DEFAULT_HASH_SIZE 32
struct CVImage {UINT64 synthetic_hash;};
UINT64 vnHashImage64(CONST CVImage& image){return image.synthetic_hash;}
'''+fn+'''
int main(){
 assert(vnCompareImages64({0},{0})==1.0f);
 std::cout<<"PASS identical synthetic hashes return1\\n";
 assert(vnCompareImages64({0},{UINT64_MAX})==0.75f);
 std::cout<<"PASS opposite64bit hashes return0.75 (64of256 denominator)\\n";
 assert(vnCompareImages64({0},{1})==255.0f/256.0f);
 std::cout<<"PASS one-bit change returns255/256 rather than63/64\\n";
}
''')
