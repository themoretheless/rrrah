#include <stdio.h>
#define STBI_NO_SIMD 1
#define STB_IMAGE_IMPLEMENTATION
#include "stb_image.h"
int main(int argc,char **argv){if(argc!=2)return 2;int x,y,n;unsigned char *p=stbi_load(argv[1],&x,&y,&n,4);if(!p)return 3;fwrite(p,4,x*y,stdout);stbi_image_free(p);return 0;}
