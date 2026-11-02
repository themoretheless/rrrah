#define XXH_INLINE_ALL
#include "xxhash.h"
#include <assert.h>
#include <stdio.h>
int main(void){unsigned char b[80];for(size_t i=0;i<sizeof b;i++)b[i]=(unsigned char)(i*37+11);unsigned cases=0;for(size_t off=0;off<8;off++)for(size_t n=0;n<=65;n++){XXH32_state_t *s=XXH32_createState();XXH64_state_t *t=XXH64_createState();assert(s&&t);XXH32_reset(s,123);XXH64_reset(t,123);for(size_t j=0;j<n;j++){assert(XXH32_update(s,b+off+j,1)==XXH_OK);assert(XXH64_update(t,b+off+j,1)==XXH_OK);}assert(XXH32_digest(s)==XXH32(b+off,n,123));assert(XXH64_digest(t)==XXH64(b+off,n,123));XXH32_freeState(s);XXH64_freeState(t);cases++;}printf("%u offset/length cases passed for XXH32 and XXH64\n",cases);}
