#include "internal.h"
#include "stb_image_resize2.h"
#include <stdio.h>
#include <string.h>
int main(void) {
    unsigned char input[33*17], named[64], mitchell[64], box[64];
    for(int i=0;i<33*17;i++)input[i]=(unsigned char)((i*73+i/11)%256);
    ph_resize_lanczos(input,33,17,named,8,8);
    if(!stbir_resize(input,33,17,0,mitchell,8,8,0,STBIR_1CHANNEL,STBIR_TYPE_UINT8,STBIR_EDGE_CLAMP,STBIR_FILTER_MITCHELL))return 1;
    ph_resize_box(input,33,17,box,8,8);
    printf("named_lanczos_equals_mitchell=%d differs_from_box=%d\n",!memcmp(named,mitchell,64),memcmp(named,box,64)!=0);
    return memcmp(named,mitchell,64)!=0 || memcmp(named,box,64)==0;
}
