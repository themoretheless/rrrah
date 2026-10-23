#include "io.hpp"
#include <cassert>
#include <iostream>
int main(){
 auto a=io::parse_hash_json(R"({"algorithm":"phash","hash_size":8,"hash_bits":64,"files":{"a":"00","b":""}})");
 assert(a.hashes.size()==2 && a.hashes[0].bits.size()==1 && a.hashes[1].bits.empty());
 auto p=io::parse_project_json(R"({"files":["a\u0020b"],"points":[[1,2]],"dims":2})");
 assert(p.files[0]=="au0020b");
 auto q=io::parse_hash_json(R"({"algorithm":"phash","hash_size":2,"hash_bits":4,"files":{"a\\b":"00"}})");
 assert(q.files[0]=="a\\\\b");
 std::cout<<"accepted64-bit hashes with1/0bytes; Unicode escape becomes "<<p.files[0]<<"; escaped backslash remains doubled\n";
}
