// Original hash.cpp, no body changes; explicit pixel IO adapter, no Magick/Elixir runtime.
#include <cstdint>
#include <iostream>
#include <string>
#include <vector>
#include <stdexcept>
#include "hash.h"
template<class T,int X> void run(const std::string& type){
 std::vector<T> actual,expected;
 if(type=="a"){
  actual=ExPerHash::a_hash<T,X>("x");expected.assign(X,T((std::uint64_t(1)<<(X/2))-1));
 }else if(type=="d"){
  actual=ExPerHash::d_hash<T,X>("x");expected.assign(X,T(~T(0)));
 }else if(type=="dd"){
  actual=ExPerHash::dd_hash<T,X>("x");expected.assign(2*X,T(0));for(int i=0;i<X;++i)expected[i]=T(~T(0));
 }else throw std::runtime_error("unknown algorithm");
 if(actual!=expected)throw std::runtime_error("independent integer oracle mismatch");
 if(type=="a")for(auto tone:{"black","white"})for(T word:ExPerHash::a_hash<T,X>(tone))if(word!=0)throw std::runtime_error("flat aHash oracle");
 std::cout<<"PASS "<<type<<" width="<<X<<" words="<<actual.size()<<"\n";
}
int main(int argc,char**argv){
 if(argc!=3)return 2;std::string type=argv[1];int size=std::stoi(argv[2]);
 if(size==8)run<std::uint8_t,8>(type);else if(size==16)run<std::uint16_t,16>(type);else if(size==32)run<std::uint32_t,32>(type);else if(size==64)run<std::uint64_t,64>(type);else return 3;
}
