#define main upstream_estimator_main
#include "dedupe_estimator.cpp"
#undef main
#include <iomanip>
int main(int argc,char**argv) {
 std::string mode=argv[1];
 if(mode=="tail") {
  std::ifstream f(argv[2],std::ios::binary);chunk_ctr_type chunks;std::vector<size_t>seen;auto bytes=add_chunks(f,chunks,0,seen,false);
  if(bytes!=3||chunks.size()!=1)return 2;
  std::cout<<"FINAL_COMPRESSED="<<std::hex<<chunks.begin()->second.compressed_len<<"\n";return 0;
 }
 if(mode=="collision") {
  std::string bytes="ABC";chunk_ctr_type chunks;chunks[std::hash<std::string>()(bytes)]={999,777,7};std::vector<size_t>seen;std::ifstream f(argv[2],std::ios::binary);auto total=add_chunks(f,chunks,0,seen,false);
  if(total!=3||chunks.size()!=1||chunks.begin()->second.len!=999||seen!=std::vector<size_t>{7})return 3;
  std::cout<<"SEEDED_HASH_COLLISION_TRUSTED_WITHOUT_BYTES\n";return 0;
 }
 if(mode=="lz4") {
  int controls=0;
  for(int n:{0,1,2,3,4,7,16,31,4096,8192,131072})for(int pattern:{0,1,2}) {
   std::string s(n,'A');for(int i=0;i<n;i++)if(pattern)s[i]=(char)(pattern==1?i%251:(i*37+i/11)%256);
   std::string compressed(LZ4_compressBound(n),0),out(n+1,0);int c=LZ4_compress_default(s.data(),compressed.data(),n,compressed.size());int r=LZ4_decompress_safe(compressed.data(),out.data(),c,n);
   if(c<=0||r!=n||out.substr(0,n)!=s)return 4;
   if(get_lz4_compressed_size(s)!=(size_t)c)return 5;
   std::string split[4];for(int i=0;i<n;i++)split[i%4].push_back(s[i]);size_t expected=0;for(auto&part:split)expected+=get_lz4_compressed_size(part);
   if(get_lz4_compressed_size_split(s)!=expected)return 6;controls++;
  }
  std::cout<<"LZ4_ROUNDTRIP_AND_SPLIT_ORACLES="<<controls<<"\n";return 0;
 }
 return 7;
}
