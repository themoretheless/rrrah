#include <assert.h>
#include <stdio.h>
#include <stdint.h>
#define STBI_NEON
#define STB_IMAGE_IMPLEMENTATION
#include "stb_image.h"
int main(void){
 short coefficients[64];unsigned char scalar[104],simd[104];uint32_t state=405;int differences=0,cases=0;
 for(int mode=0;mode<3;mode++)for(int sample=0;sample<(mode==2?2048:256);sample++){
  memset(coefficients,0,sizeof coefficients);memset(scalar,0xa5,sizeof scalar);memset(simd,0xa5,sizeof simd);
  if(mode==0)coefficients[0]=(sample-128)*8;
  else if(mode==1)coefficients[sample%64]=(sample<128?-1:1)*(sample%128<64?64:256);
  else for(int k=0;k<64;k++){state=state*1664525u+1013904223u;coefficients[k]=(short)((int)(state%129)-64);}
  stbi__idct_block(scalar,13,coefficients);stbi__idct_simd(simd,13,coefficients);
  for(int i=0;i<104;i++)differences+=(scalar[i]!=simd[i]);cases++;
 }
 printf("{\"cases\":%d,\"differing_bytes\":%d,\"stride\":13,\"seed\":405}\n",cases,differences);assert(differences==0);
}
