#include <stdio.h>
#include <assert.h>
#define STBI_NO_SIMD 1
#define STB_IMAGE_IMPLEMENTATION
#include "stb_image.h"
int main(void){unsigned char ramp[]={0,255},reverse[]={255,0},a[4],b[4];stbi__resample_row_h_2(a,ramp,ramp,2,2);stbi__resample_row_h_2(b,reverse,reverse,2,2);assert(a[0]==0&&a[1]==64&&a[2]==64&&a[3]==255);int mismatches=0;for(int i=0;i<4;i++)if(a[i]!=b[3-i])mismatches++;printf("{\"forward\":[%u,%u,%u,%u],\"reverse\":[%u,%u,%u,%u],\"independent_expected_forward\":[0,64,191,255],\"reflection_mismatches\":%d}\n",a[0],a[1],a[2],a[3],b[0],b[1],b[2],b[3],mismatches);return 0;}
