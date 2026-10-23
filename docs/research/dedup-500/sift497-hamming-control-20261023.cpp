#include "src/hash/hash.hpp"
#include <algorithm>
#include <cassert>
#include <iostream>
int HashResult::hamming(const HashResult& a, const HashResult& b) {
    int dist = 0;
    int len = (int)std::min(a.bits.size(), b.bits.size());
    for (int i = 0; i < len; i++) {
        dist += __builtin_popcount(a.bits[i] ^ b.bits[i]);
    }
    return dist;
}

int main(){HashResult a{{0},2},b{{0,255},4};assert(HashResult::hamming(a,b)==0);std::cout<<"different-size hashes compare as distance0; trailing byte ignored\n";HashResult c{{0x80,0},3},d{{0x80,1},3};assert(HashResult::hamming(c,d)==1);std::cout<<"unused padding bit contributes distance1 for9-bit hashes\n";}
