#include <stdio.h>
#include <stdlib.h>
#define STB_IMAGE_RESIZE_IMPLEMENTATION
#include "stb_image_resize.h"
int main(int argc,char **argv){
 if(argc!=5)return 2;
 int w=atoi(argv[1]),h=atoi(argv[2]),ow=atoi(argv[3]),oh=atoi(argv[4]);
 unsigned char *in=malloc(w*h),*out=malloc(ow*oh);if(!in||!out)return 3;
 for(int i=0;i<w*h;i++)in[i]=(i*71+w*23+h*11)%256;
 int ok=stbir_resize_uint8_generic(in,w,h,0,out,ow,oh,0,1,-1,0,STBIR_EDGE_CLAMP,STBIR_FILTER_TRIANGLE,STBIR_COLORSPACE_LINEAR,NULL);
 free(in);free(out);return ok?0:4;
}
