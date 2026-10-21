#include <assert.h>
#include <math.h>
#include <stdio.h>
#define STB_IMAGE_IMPLEMENTATION
#include "stb_image.h"
int main(void){
 short coefficients[64]={0};unsigned char pixels[64];int cases=0,maxerr=0;
 for(int dc=-1024;dc<=1016;dc+=8){memset(coefficients,0,sizeof coefficients);coefficients[0]=dc;stbi__idct_block(pixels,8,coefficients);for(int i=0;i<64;i++)assert(pixels[i]==128+dc/8);cases++;}
 for(int k=0;k<64;k++)for(int sign=-1;sign<=1;sign+=2){memset(coefficients,0,sizeof coefficients);coefficients[k]=64*sign;stbi__idct_block(pixels,8,coefficients);
  int u=k%8,v=k/8;double au=u?1:M_SQRT1_2,av=v?1:M_SQRT1_2;
  for(int y=0;y<8;y++)for(int x=0;x<8;x++){
   double sample=128+0.25*au*av*coefficients[k]*cos((2*x+1)*u*M_PI/16)*cos((2*y+1)*v*M_PI/16);
   int ref=(int)floor(sample+.5);if(ref<0)ref=0;if(ref>255)ref=255;int e=abs((int)pixels[y*8+x]-ref);if(e>maxerr)maxerr=e;assert(e<=1);
  }cases++;
 }
 printf("{\"cases\":%d,\"dc_cases\":256,\"signed_single_basis_cases\":128,\"maximum_byte_error\":%d,\"oracle\":\"independent cosine IDCT basis\"}\n",cases,maxerr);
}
