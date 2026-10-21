#include <stdio.h>
#include <assert.h>
#define STB_IMAGE_RESIZE_IMPLEMENTATION
#include "stb_image_resize.h"
int main(void){
 unsigned char mixed[]={255,0,0,0,0,0,255,255},hidden[]={255,0,0,0,0,0,255,0},a[4],b[4];
 float hidden_float[]={1,0,0,0,0,0,1,0},c[4];
 assert(stbir_resize_uint8_generic(mixed,2,1,0,a,1,1,0,4,3,0,STBIR_EDGE_CLAMP,STBIR_FILTER_TRIANGLE,STBIR_COLORSPACE_LINEAR,NULL));
 assert(a[0]==0&&a[1]==0&&a[2]==255&&a[3]==128);
 assert(stbir_resize_uint8_generic(hidden,2,1,0,b,1,1,0,4,3,0,STBIR_EDGE_CLAMP,STBIR_FILTER_TRIANGLE,STBIR_COLORSPACE_LINEAR,NULL));
 assert(b[0]==128&&b[1]==0&&b[2]==128&&b[3]==0);
 assert(stbir_resize_float_generic(hidden_float,2,1,0,c,1,1,0,4,3,0,STBIR_EDGE_CLAMP,STBIR_FILTER_TRIANGLE,STBIR_COLORSPACE_LINEAR,NULL));
 for(int i=0;i<4;i++)assert(c[i]==0);
 printf("{\"mixed_rgba8\":[%u,%u,%u,%u],\"all_transparent_rgba8\":[%u,%u,%u,%u],\"all_transparent_float\":[0,0,0,0]}\n",a[0],a[1],a[2],a[3],b[0],b[1],b[2],b[3]);
}
