#include <assert.h>
#include <stdio.h>
#include "pHash.c"
int main(void){
 unsigned char rgb[3]={0};PhashImage *im=NULL;
 PhashError invalid=phash_image_create(rgb,0,0,3,false,&im);assert(invalid==PHASH_OK);phash_image_destroy(im);
 PhashConfig c=phash_config_default();c.hash_size=1;int accepts_one=phash_config_validate(&c);uint64_t h=123;PhashImage good={rgb,1,1,3,false};int computes_one=phash_compute(&good,&c,&h);assert(accepts_one==0&&computes_one==PHASH_ERR_DOMAIN);
 int untouched=-1;
 #if defined(__aarch64__)
 double input[64]={0},output[64];for(int i=0;i<64;i++)output[i]=1234567.;dct_8x8_neon(input,output);untouched=0;for(int i=0;i<64;i++)untouched+=(output[i]==1234567.);assert(untouched==48);
 #endif
 printf("{\"zero_dimension_create_status\":%d,\"hash_size_one_validate\":%d,\"hash_size_one_compute\":%d,\"neon_unwritten_coefficients\":%d}\n",invalid,accepts_one,computes_one,untouched);phash_terminate();
}
