#include <stdio.h>
#include <assert.h>
#define STBI_NO_SIMD 1
#define STB_IMAGE_IMPLEMENTATION
#include "stb_image.h"
int main(int argc,char **argv){
 assert(argc==3);int x,y,n;
 stbi_us *a=stbi_load_16(argv[1],&x,&y,&n,1),*b=stbi_load_16(argv[2],&x,&y,&n,1);assert(a&&b&&a[0]==0x1200&&b[0]==0x12ff);
 unsigned char *c=stbi_load(argv[1],&x,&y,&n,1),*d=stbi_load(argv[2],&x,&y,&n,1);assert(c&&d&&c[0]==0x12&&d[0]==0x12);
 float *e=stbi_loadf(argv[1],&x,&y,&n,1),*f=stbi_loadf(argv[2],&x,&y,&n,1);assert(e&&f&&e[0]==f[0]);
 printf("{\"decoded16\":[%u,%u],\"decoded8\":[%u,%u],\"decoded_float\":[%.9g,%.9g],\"float_collision\":true}\n",a[0],b[0],c[0],d[0],e[0],f[0]);
 stbi_image_free(a);stbi_image_free(b);stbi_image_free(c);stbi_image_free(d);stbi_image_free(e);stbi_image_free(f);return 0;
}
