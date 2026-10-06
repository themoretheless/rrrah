/* Test-only host for the unchanged official PackComponentsToRAW. */
#include "common.h"
#define DIMENSION int
#define ENABLED_PARTS int
typedef int16_t COMPONENT_VALUE;
typedef struct {COMPONENT_VALUE *data;size_t pitch;} COMPONENT_ARRAY;
typedef struct {COMPONENT_ARRAY *component_array_list;} UNPACKED_IMAGE;
#include "pack-original.inc"
static void word(FILE *out,uint16_t v) {fputc(v&255,out);fputc(v>>8,out);}

static int16_t readword(FILE *in) {int a=fgetc(in),b=fgetc(in);if(a<0||b<0)exit(7);return (int16_t)(a|(b<<8));}
int main(int argc,char **argv) {
 if(argc!=8)return 2;int w=atoi(argv[1]),h=atoi(argv[2]);if(w<=0||h<=0)return 3;
 COMPONENT_ARRAY arrays[4];
 for(int c=0;c<4;++c) {
  FILE *in=fopen(argv[c+3],"rb");if(!in)return 4;
  arrays[c].data=malloc((size_t)w*h*2);arrays[c].pitch=w*2;
  for(int i=0;i<w*h;++i)arrays[c].data[i]=readword(in);
  if(fgetc(in)!=EOF)return 5;fclose(in);
 }
 PIXEL *sensor=malloc((size_t)w*h*8);UNPACKED_IMAGE image={arrays};
 SetupDecoderLogCurve();PackComponentsToRAW(&image,sensor,w*8,w,h,0,14,PIXEL_FORMAT_RAW_RGGB_14);
 FILE *out=fopen(argv[7],"wb");if(!out)return 6;
 for(int i=0;i<w*h*4;++i)word(out,(uint16_t)sensor[i]);
 for(int c=0;c<4;++c)free(arrays[c].data);free(sensor);return fclose(out)?8:0;
}
