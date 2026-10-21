#include <stdio.h>
#include <assert.h>
#include <math.h>
#include <stdlib.h>
#define STBI_NO_SIMD 1
#define STB_IMAGE_IMPLEMENTATION
#include "stb_image.h"
int main(void){short block[64]={0};unsigned char out[64];int max_error=0,cases=0;
 for(int v=0;v<256;v++){memset(block,0,sizeof block);block[0]=(v-128)*8;stbi__idct_block(out,8,block);for(int p=0;p<64;p++)assert(out[p]==v);}
 const int values[]={-32,-8,0,8,32};
 for(int k=1;k<64;k++)for(int j=0;j<5;j++){
  memset(block,0,sizeof block);block[k]=values[j];stbi__idct_block(out,8,block);int u=k%8,v=k/8;double norm=(u?1:1/sqrt(2.0))*(v?1:1/sqrt(2.0));
  for(int y=0;y<8;y++)for(int x=0;x<8;x++){
   double value=128+0.25*norm*values[j]*cos((2*x+1)*u*acos(-1)/16)*cos((2*y+1)*v*acos(-1)/16);int expected=(int)floor(value+0.5);if(expected<0)expected=0;if(expected>255)expected=255;int error=abs(out[y*8+x]-expected);if(error>max_error)max_error=error;assert(error<=1);
  }cases++;
 }
 printf("{\"exact_dc_blocks\":256,\"single_ac_blocks\":%d,\"pixels_per_block\":64,\"max_ac_byte_error\":%d}\n",cases,max_error);return 0;}
