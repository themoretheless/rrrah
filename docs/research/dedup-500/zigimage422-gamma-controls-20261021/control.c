#include <stdio.h>
#include <assert.h>
#include <math.h>
#define STBI_NO_SIMD 1
#define STB_IMAGE_IMPLEMENTATION
#include "stb_image.h"
int main(void){
 const unsigned char ppm[]="P6\n1 1\n255\n\x80\x80\x80";
 int x,y,n;float *a=stbi_loadf_from_memory(ppm,sizeof(ppm)-1,&x,&y,&n,3);assert(a);float before=a[0];assert(fabs(before-pow(128.0/255,2.2))<1e-6);stbi_image_free(a);
 stbi_ldr_to_hdr_gamma(1.0f);a=stbi_loadf_from_memory(ppm,sizeof(ppm)-1,&x,&y,&n,3);assert(a);float after=a[0];assert(fabs(after-128.0/255)<1e-6);assert(after!=before);stbi_image_free(a);stbi_ldr_to_hdr_gamma(2.2f);
 const unsigned char hdr[]="#?RADIANCE\nFORMAT=32-bit_rle_rgbe\n\n-Y 1 +X 1\n\x80\x40\x20\x81";
 unsigned char *b=stbi_load_from_memory(hdr,sizeof(hdr)-1,&x,&y,&n,3);assert(b);unsigned char default_rgb[3]={b[0],b[1],b[2]};stbi_image_free(b);
 stbi_hdr_to_ldr_gamma(1);b=stbi_load_from_memory(hdr,sizeof(hdr)-1,&x,&y,&n,3);assert(b);assert(b[0]==255&&b[1]==128&&b[2]==64);assert(default_rgb[1]!=b[1]);
 printf("{\"ldr_float_default\":%.9g,\"ldr_float_gamma1\":%.9g,\"hdr_byte_default\":[%u,%u,%u],\"hdr_byte_gamma1\":[%u,%u,%u]}\n",before,after,default_rgb[0],default_rgb[1],default_rgb[2],b[0],b[1],b[2]);stbi_image_free(b);stbi_hdr_to_ldr_gamma(2.2f);return 0;
}
