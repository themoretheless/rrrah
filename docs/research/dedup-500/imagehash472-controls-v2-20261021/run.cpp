#include <algorithm>
#include <cassert>
#include <iostream>
#include "imghash.h"
int main(){
 using namespace imghash;
 Hasher::hash_type a{0},b{},c{255};
 assert(Hasher::match(a,b)&&!Hasher::equal(a,b));
 std::cout<<"PASS empty hash prefix matches nonempty by documented prefix policy, strict equal rejects\n";
 assert(Hasher::distance(a,b)==0&&Hasher::distance(b,c)==0&&Hasher::distance(a,c)==8);
 std::cout<<"PASS variable-length prefix distance violates triangle inequality; not a metric for unrestricted mixed lengths\n";
 Hasher::hash_type zeros(32,0),ones(32,255);assert(Hasher::distance(zeros,ones)==256);
 std::cout<<"PASS equal-length full256bit Hamming distance\n";
 Image<uint8_t> black(1,1,3),white(1,1,3);std::fill(black.begin(),black.end(),0);std::fill(white.begin(),white.end(),255);
 Preprocess p(32,32);auto x=p.apply(black);auto y=p.apply(white);
 assert(std::equal(x.begin(),x.end(),y.begin(),y.end()));BlockHasher h;assert(h.apply(x)==h.apply(y));
 std::cout<<"PASS original preprocessing and block hash collapse solid black/white; not content identity\n";
 std::cout<<"RESULT 4 original C++ controls passed; no encoded decoder, MVP database or sanitizer run\n";
}
