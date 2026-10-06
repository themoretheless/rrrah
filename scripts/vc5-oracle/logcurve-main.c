/* Execute the unchanged official GoPro SetupDecoderLogCurve. */
#include "common.h"
int main(int argc,char **argv) {
    if(argc!=2)return 2;
    FILE *out=fopen(argv[1],"wb");if(!out)return 3;
    SetupDecoderLogCurve();
    for(int i=0;i<4096;++i) {uint16_t v=DecoderLogCurve[i];fputc(v&255,out);fputc(v>>8,out);}
    return fclose(out)?4:0;
}
