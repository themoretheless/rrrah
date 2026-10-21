#include <stdio.h>
#include <stdint.h>
#define STBI_NO_SIMD 1
#define STB_IMAGE_IMPLEMENTATION
#include "stb_image.h"
int main(int argc,char **argv){printf("[");for(int i=1;i<argc;i++){int x=0,y=0,n=0;unsigned char *p=stbi_load(argv[i],&x,&y,&n,3);uint64_t sum=14695981039346656037ULL;if(p)for(int j=0;j<x*y*3;j++)sum=(sum^p[j])*1099511628211ULL;printf("%s{\"loaded\":%s,\"width\":%d,\"height\":%d,\"rgb_fnv64\":\"%016llx\"}",i==1?"":",",p?"true":"false",x,y,(unsigned long long)sum);stbi_image_free(p);}puts("]");return 0;}
