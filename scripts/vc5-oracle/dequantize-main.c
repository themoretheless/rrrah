/* External test oracle; link unmodified GoPro companding.c/dequantize.c. */
#include "headers.h"
PIXEL DequantizedValue(int32_t value, int quantization);
int main(int argc, char **argv) {
    if (argc!=2) return 2;
    FILE *out=fopen(argv[1],"wb"); if (!out) return 3;
    const int factors[]={0,1,12,24,48,96,144,65535};
    for (size_t q=0;q<sizeof(factors)/sizeof(factors[0]);++q)
        for (int value=-255;value<=255;++value) {
            uint16_t sample=(uint16_t)DequantizedValue(value,factors[q]);
            uint8_t bytes[2]={(uint8_t)sample,(uint8_t)(sample>>8)};
            if (fwrite(bytes,1,2,out)!=2) return 4;
        }
    return fclose(out)?5:0;
}
