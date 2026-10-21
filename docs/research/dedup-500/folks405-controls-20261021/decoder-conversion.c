#include <assert.h>
#include <stdio.h>
#define STB_IMAGE_IMPLEMENTATION
#include "stb_image.h"
int main(void){
 stbi__uint16 *v=malloc(65536*sizeof(*v));assert(v);for(int i=0;i<65536;i++)v[i]=(stbi__uint16)i;
 unsigned char *reduced=stbi__convert_16_to_8(v,65536,1,1);assert(reduced);for(int i=0;i<65536;i++)assert(reduced[i]==i/256);free(reduced);
 unsigned char *small=malloc(256);assert(small);for(int i=0;i<256;i++)small[i]=(unsigned char)i;
 stbi__uint16 *expanded=stbi__convert_8_to_16(small,256,1,1);assert(expanded);for(int i=0;i<256;i++)assert(expanded[i]==i*257);free(expanded);
 unsigned char *rgba=malloc(4);rgba[0]=255;rgba[1]=0;rgba[2]=0;rgba[3]=0;
 unsigned char *rgb=stbi__convert_format(rgba,4,3,1,1);assert(rgb);assert(rgb[0]==255&&rgb[1]==0&&rgb[2]==0);free(rgb);
 printf("{\"all_16bit_values_checked\":65536,\"all_8bit_values_checked\":256,\"transparent_red_becomes_rgb\":[255,0,0],\"conversion\":\"16to8 discards low byte;8to16 replicates byte\"}\n");
}
