#include <stdio.h>
#include <assert.h>
#define STBI_NO_SIMD 1
#define STB_IMAGE_IMPLEMENTATION
#include "stb_image.h"
int main(int argc,char **argv){assert(argc==2);int x,y,n;unsigned char *a=stbi_load(argv[1],&x,&y,&n,4);assert(a&&x==2&&y==1&&n==4&&a[3]==0&&a[7]==0);unsigned char *b=stbi_load(argv[1],&x,&y,&n,3);assert(b&&n==4&&b[0]==255&&b[1]==0&&b[2]==0&&b[3]==0&&b[4]==0&&b[5]==255);printf("{\"source_alpha\":[%u,%u],\"forced_rgb_pixels\":[[%u,%u,%u],[%u,%u,%u]],\"source_channels_reported\":%d}\n",a[3],a[7],b[0],b[1],b[2],b[3],b[4],b[5],n);stbi_image_free(a);stbi_image_free(b);return 0;}
