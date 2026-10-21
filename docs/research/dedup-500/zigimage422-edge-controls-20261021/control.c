#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#define STB_IMAGE_RESIZE_IMPLEMENTATION
#include "stb_image_resize.h"
int main(void){
 unsigned char input[9],reverse[9],out[11],back[11];int checked=0,max_delta=0;
 for(int n=1;n<=9;n++){
  for(int i=0;i<n;i++)input[i]=(unsigned char)((i*71+n*23)%256);
  for(int i=0;i<n;i++)reverse[i]=input[n-1-i];
  for(int m=1;m<=11;m++){
   fprintf(stderr,"pair %d -> %d\n",n,m);
   assert(stbir_resize_uint8_generic(input,n,1,0,out,m,1,0,1,-1,0,STBIR_EDGE_CLAMP,STBIR_FILTER_TRIANGLE,STBIR_COLORSPACE_LINEAR,NULL));
   assert(stbir_resize_uint8_generic(reverse,n,1,0,back,m,1,0,1,-1,0,STBIR_EDGE_CLAMP,STBIR_FILTER_TRIANGLE,STBIR_COLORSPACE_LINEAR,NULL));
   for(int i=0;i<m;i++){int delta=abs(out[i]-back[m-1-i]);if(delta>max_delta)max_delta=delta;assert(delta<=1);}
   if(n==m)for(int i=0;i<n;i++)assert(out[i]==input[i]);
   checked++;
  }
 }
 printf("{\"reversal_dimension_pairs\":%d,\"identity_sizes\":9,\"max_byte_reversal_difference\":%d}\n",checked,max_delta);
}
