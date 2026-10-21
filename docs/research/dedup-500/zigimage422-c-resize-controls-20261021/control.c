#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include <assert.h>
static int fail_alloc=0,allocs=0,frees=0;
static void *test_alloc(size_t n){allocs++;return fail_alloc?NULL:malloc(n);}
static void test_free(void *p){if(p)frees++;free(p);}
#define STBIR_MALLOC(n,c) test_alloc(n)
#define STBIR_FREE(p,c) test_free(p)
#define STB_IMAGE_RESIZE_IMPLEMENTATION
#include "stb_image_resize.h"
static int resize_gray(const unsigned char *in,int w,int h,unsigned char *out,int ow,int oh,stbir_colorspace s){return stbir_resize_uint8_generic(in,w,h,0,out,ow,oh,0,1,-1,0,STBIR_EDGE_CLAMP,STBIR_FILTER_TRIANGLE,s,NULL);}
int main(void){
 const int dims[][4]={{1,1,8,8},{3,5,9,8},{31,33,8,8},{64,64,32,32}};
 unsigned char in[4096],out[4096];int cases=0;
 const unsigned char vals[]={0,73,255};
 for(int d=0;d<4;d++)for(int v=0;v<3;v++){memset(in,vals[v],sizeof(in));memset(out,17,sizeof(out));assert(resize_gray(in,dims[d][0],dims[d][1],out,dims[d][2],dims[d][3],STBIR_COLORSPACE_LINEAR)==1);for(int i=0;i<dims[d][2]*dims[d][3];i++)assert(out[i]==vals[v]);cases++;}
 int before_alloc=allocs,before_free=frees;fail_alloc=1;memset(out,91,sizeof(out));assert(resize_gray(in,3,3,out,8,8,STBIR_COLORSPACE_LINEAR)==0);for(int i=0;i<64;i++)assert(out[i]==91);assert(allocs-before_alloc==1 && frees==before_free);fail_alloc=0;
 unsigned char pair[]={0,255},linear=0,srgb=0;assert(resize_gray(pair,2,1,&linear,1,1,STBIR_COLORSPACE_LINEAR)==1);assert(resize_gray(pair,2,1,&srgb,1,1,STBIR_COLORSPACE_SRGB)==1);assert(linear==128);assert(srgb==188);assert(allocs==frees+1);
 printf("{\"constant_cases\":%d,\"allocation_failure_returns_zero\":true,\"failed_output_unchanged\":true,\"linear_black_white\":%u,\"srgb_black_white\":%u,\"alloc_attempts\":%d,\"successful_frees\":%d}\n",cases,linear,srgb,allocs,frees);return 0;
}
