#include "stb_image_resize2.h"
#include <stdio.h>
int main(void) {
    unsigned char input[32*32]={0},output[64*64]={0};
    STBIR_RESIZE resize;
    stbir_resize_init(&resize,input,32,32,0,output,64,64,0,STBIR_1CHANNEL,STBIR_TYPE_UINT8);
    int first=stbir_build_samplers_with_splits(&resize,4);
    int second=stbir_build_samplers_with_splits(&resize,4);
    printf("first_count=%d repeated_count=%d actual_stored_splits=%d\n",first,second,resize.splits);
    int confirmed=(first==4 && second==1 && resize.splits==4);
    stbir_free_samplers(&resize);
    return !confirmed;
}
