/* Test-only host for the unchanged official PackComponentsToRAW. */
#include "common.h"
#define DIMENSION int
#define ENABLED_PARTS int
typedef int16_t COMPONENT_VALUE;
typedef struct {COMPONENT_VALUE *data;size_t pitch;} COMPONENT_ARRAY;
typedef struct {COMPONENT_ARRAY *component_array_list;} UNPACKED_IMAGE;
#include "pack-original.inc"
static void word(FILE *out,uint16_t v) {fputc(v&255,out);fputc(v>>8,out);}
int main(int argc,char **argv) {
 if(argc!=2)return 2;FILE *out=fopen(argv[1],"wb");if(!out)return 3;
 SetupDecoderLogCurve();
 for(int bits=12;bits<=16;bits+=2)for(int test=0;test<256;++test) {
  COMPONENT_VALUE c[4]; COMPONENT_ARRAY a[4];PIXEL sensor[4];
  for(int i=0;i<4;++i) {c[i]=(test*997+i*7919)%16384;a[i].data=&c[i];a[i].pitch=2;}
  if(test%16==0)for(int i=0;i<4;++i)c[i]=32767;
  if(test%16==1)for(int i=0;i<4;++i)c[i]=-32768;
  UNPACKED_IMAGE image={a};
  PackComponentsToRAW(&image,sensor,8,1,1,0,bits,PIXEL_FORMAT_RAW_RGGB_14);
  word(out,bits);for(int i=0;i<4;++i)word(out,(uint16_t)c[i]);
  for(int i=0;i<4;++i)word(out,(uint16_t)sensor[i]);
 }
 return fclose(out)?4:0;
}
