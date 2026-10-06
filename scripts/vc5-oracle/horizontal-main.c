/* Oracle host: horizontal-original.inc is an unchanged function from pinned inverse.c. */
#include "headers.h"
#undef absolute
#include "macros.h"
#define STATIC static
#define DIMENSION int
static const int32_t rounding=4;
#include "horizontal-original.inc"
#include "horizontal-descale-original.inc"
static void word(FILE *out,int16_t v) {
    uint16_t u=(uint16_t)v; fputc(u&255,out); fputc(u>>8,out);
}
int main(int argc,char **argv) {
    if(argc!=2 && argc!=3)return 2;
    FILE *out=fopen(argv[1],"wb");if(!out)return 3;
    for(int test=0;test<256;++test) {
        int n=3+test%8, width=2*n-test%2;
        PIXEL low[10],high[10],output[20];
        for(int x=0;x<n;++x) {
            low[x]=(test*113+x*631)%65536-32768;
            high[x]=(test*997+x*7919)%65536-32768;
            if(test%16==0) { low[x]=32767; high[x]=32767; }
            if(test%16==1) { low[x]=32767; high[x]=-32768; }
        }
        if(argc==3) InvertHorizontalDescale16s(low,high,output,n,width,2);
        else InvertHorizontal16s(low,high,output,n,width);
        word(out,n);word(out,width);
        for(int x=0;x<n;++x)word(out,low[x]);
        for(int x=0;x<n;++x)word(out,high[x]);
        for(int x=0;x<width;++x)word(out,output[x]);
    }
    return fclose(out)?4:0;
}
