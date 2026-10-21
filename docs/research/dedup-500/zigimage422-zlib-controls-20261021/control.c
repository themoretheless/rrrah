#include <stdio.h>
#include <stdlib.h>
#define STBI_NO_SIMD 1
#define STB_IMAGE_IMPLEMENTATION
#include "stb_image.h"
int main(int argc,char **argv){if(argc!=2)return 2;FILE *f=fopen(argv[1],"rb");if(!f)return 3;unsigned char input[8192];int n=(int)fread(input,1,sizeof input,f);fclose(f);char out[8192];int size=stbi_zlib_decode_buffer(out,sizeof out,(const char *)input,n);if(size<0)return 4;fwrite(out,1,size,stdout);return 0;}
