/* Test-only adapter: compile against pinned, unmodified GoPro vlc.c/table17.inc. */
#include "headers.h"
#include "table17.inc"
int main(int argc, char **argv) {
    if (argc != 4) return 2;
    FILE *input = fopen(argv[1], "rb"), *out = fopen(argv[3], "wb");
    if (!input || !out) return 3;
    fseek(input,0,SEEK_END); long length=ftell(input); rewind(input);
    if (length < 0) return 4;
    uint8_t *bytes=malloc((size_t)length); if (!bytes) return 5;
    if (fread(bytes,1,(size_t)length,input)!=(size_t)length) return 6;
    BITSTREAM stream={bytes,(size_t)length,0};
    size_t remaining=strtoull(argv[2],NULL,10);
    while (remaining) {
        RUN run;
        if (GetRun(&stream,(CODEBOOK*)&table17,&run) || !run.count || run.count>remaining) return 8;
        for (uint32_t i=0;i<run.count;++i) {
            uint32_t v=(uint32_t)run.value;
            uint8_t sample[4]={(uint8_t)v,(uint8_t)(v>>8),(uint8_t)(v>>16),(uint8_t)(v>>24)};
            if (fwrite(sample,1,4,out)!=4) return 9;
        }
        remaining-=run.count;
    }
    RUN end;
    if (GetRlv(&stream,(CODEBOOK*)&table17,&end) || end.count || end.value!=1) return 10;
    free(bytes); fclose(input); return fclose(out) ? 11 : 0;
}
