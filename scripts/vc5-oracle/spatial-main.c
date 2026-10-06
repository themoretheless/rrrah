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
int main(int argc,char **argv) {
    if(argc!=2 && argc!=3)return 2;
    FILE *out=fopen(argv[1],"wb");if(!out)return 3;
    gpr_allocator allocator={malloc,free};
    for(int test=0;test<32;++test) {
        int w=3+test%4,h=3+(test/4)%4,ow=2*w-test%2,oh=2*h-(test/2)%2;
        PIXEL bands[4][36],output[144];int q[4]={1,12,24,96};
        for(int b=0;b<4;++b)for(int i=0;i<w*h;++i)
            bands[b][i]=b==0 ? (test*113+i*631)%32768 : (test*17+i*29+b*31)%511-255;
        if(argc==3) InvertSpatialQuantDescale16s(&allocator,bands[0],w*2,bands[1],w*2,bands[2],w*2,bands[3],w*2,
            output,ow*2,w,h,ow,oh,2,q);
        else InvertSpatialQuant16s(&allocator,bands[0],w*2,bands[1],w*2,bands[2],w*2,bands[3],w*2,
            output,ow*2,w,h,ow,oh,q);
        word(out,w);word(out,h);word(out,ow);word(out,oh);
        for(int b=0;b<4;++b)for(int i=0;i<w*h;++i)word(out,bands[b][i]);
        for(int i=0;i<ow*oh;++i)word(out,output[i]);
    }
    return fclose(out)?4:0;
}
