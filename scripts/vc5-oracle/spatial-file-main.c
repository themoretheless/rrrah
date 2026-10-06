/* Test-only wrapper for unchanged pinned GoPro inverse filter. */
#include "headers.h"
#undef absolute
#include "macros.h"
#define STATIC static
#define DIMENSION int
#define QUANT int
#define LH_BAND 1
#define HL_BAND 2
#define HH_BAND 3
static const int32_t rounding=4;
typedef struct { void *(*Alloc)(size_t); void (*Free)(void*); } gpr_allocator;
CODEC_ERROR DequantizeBandRow16s(PIXEL*,int,int,PIXEL*);
#include "horizontal-original.inc"
#include "horizontal-descale-original.inc"
#include "spatial-original.inc"
#include "spatial-descale-original.inc"
static void word(FILE *out,int16_t v) { uint16_t u=(uint16_t)v;fputc(u&255,out);fputc(u>>8,out); }

static int readword(FILE *in) { int a=fgetc(in),b=fgetc(in);if(a<0||b<0)exit(6);return a|(b<<8); }
int main(int argc,char **argv) {
 if(argc!=3)return 2;
 FILE *in=fopen(argv[1],"rb"),*out=fopen(argv[2],"wb");if(!in||!out)return 3;
 int w=readword(in),h=readword(in),ow=readword(in),oh=readword(in),scale=readword(in);
 int q[4]={1,readword(in),readword(in),readword(in)};
 PIXEL *bands[4],*output=malloc((size_t)ow*oh*2);
 for(int b=0;b<4;++b) {bands[b]=malloc((size_t)w*h*2);for(int i=0;i<w*h;++i)bands[b][i]=(int16_t)readword(in);}
 gpr_allocator allocator={malloc,free};
 if(scale==2) InvertSpatialQuantDescale16s(&allocator,bands[0],w*2,bands[1],w*2,bands[2],w*2,bands[3],w*2,output,ow*2,w,h,ow,oh,2,q);
 else if(scale==0) InvertSpatialQuant16s(&allocator,bands[0],w*2,bands[1],w*2,bands[2],w*2,bands[3],w*2,output,ow*2,w,h,ow,oh,q);
 else return 4;
 for(int i=0;i<ow*oh;++i)word(out,output[i]);
 for(int b=0;b<4;++b)free(bands[b]);free(output);fclose(in);return fclose(out)?5:0;
}
