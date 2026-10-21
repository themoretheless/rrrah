#include <stdio.h>
#include <assert.h>
#include <string.h>
#define STBI_NO_SIMD 1
#define STB_IMAGE_IMPLEMENTATION
#include "stb_image.h"
int main(void){
 unsigned char a[]="P6\n1 1\n65535\n\x12\x00\x34\x00\x56\x00";
 unsigned char b[]="P6\n1 1\n65535\n\x12\xff\x34\xff\x56\xff";
 int x=0,y=0,n=0;unsigned char *p=stbi_load_from_memory(a,sizeof(a)-1,&x,&y,&n,3);assert(p&&x==1&&y==1&&n==3);
 unsigned char *q=stbi_load_from_memory(b,sizeof(b)-1,&x,&y,&n,3);assert(q);assert(p[0]==0&&q[0]==255);
 stbi_us *r=stbi_load_16_from_memory(a,sizeof(a)-1,&x,&y,&n,3),*s=stbi_load_16_from_memory(b,sizeof(b)-1,&x,&y,&n,3);assert(r&&s&&memcmp(r,s,6)!=0);assert(r[0]==18&&s[0]==65298);
 printf("{\"decoded8_a\":[%u,%u,%u],\"decoded8_b\":[%u,%u,%u],\"decoded16_a\":[%u,%u,%u],\"decoded16_b\":[%u,%u,%u]}\n",p[0],p[1],p[2],q[0],q[1],q[2],r[0],r[1],r[2],s[0],s[1],s[2]);
 stbi_image_free(p);stbi_image_free(q);stbi_image_free(r);stbi_image_free(s);return 0;
}
