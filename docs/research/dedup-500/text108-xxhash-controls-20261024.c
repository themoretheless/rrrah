#define XXH_INLINE_ALL
#include "xxhash.h"
#include <assert.h>
#include <stdio.h>
int main(void){unsigned char bytes[4097];for(size_t i=0;i<sizeof(bytes);i++)bytes[i]=(unsigned char)(i*37+11);size_t sizes[]={0,1,3,16,17,128,129,240,241,1024,4097};for(size_t i=0;i<sizeof(sizes)/sizeof(sizes[0]);i++){size_t n=sizes[i];XXH3_state_t *s=XXH3_createState();assert(s);assert(XXH3_64bits_reset(s)==XXH_OK);for(size_t j=0;j<n;){size_t k=n-j>7?7:n-j;assert(XXH3_64bits_update(s,bytes+j,k)==XXH_OK);j+=k;}XXH64_hash_t h=XXH3_64bits(bytes,n);assert(XXH3_64bits_digest(s)==h);XXH64_canonical_t c;XXH64_canonicalFromHash(&c,h);assert(XXH64_hashFromCanonical(&c)==h);XXH3_freeState(s);}puts("11 one-shot/streaming and canonical roundtrip cases passed");}
