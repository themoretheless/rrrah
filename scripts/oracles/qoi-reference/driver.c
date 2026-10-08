/* Test-only driver for the pinned upstream QOI reference implementation. */
#define QOI_IMPLEMENTATION
#include "qoi.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

int main(int argc, char **argv) {
    if (argc == 4 && !strcmp(argv[1], "decode")) {
        qoi_desc desc;
        void *pixels = qoi_read(argv[2], &desc, 4);
        if (!pixels) return 1;
        FILE *out = fopen(argv[3], "wb");
        if (!out) { free(pixels); return 1; }
        size_t size = (size_t)desc.width * desc.height * 4;
        int ok = fwrite(pixels, 1, size, out) == size;
        if (fclose(out)) ok = 0;
        free(pixels);
        return ok ? 0 : 1;
    }
    if (argc != 8 || strcmp(argv[1], "encode")) return 2;
    qoi_desc desc = { (unsigned)atoi(argv[4]), (unsigned)atoi(argv[5]),
        (unsigned char)atoi(argv[6]), (unsigned char)atoi(argv[7]) };
    if (!desc.width || !desc.height || desc.width > 1024 || desc.height > 1024
        || (desc.channels != 3 && desc.channels != 4) || desc.colorspace > 1) return 2;
    size_t size = (size_t)desc.width * desc.height * desc.channels;
    void *pixels = malloc(size);
    FILE *in = fopen(argv[2], "rb");
    if (!pixels || !in) { free(pixels); if (in) fclose(in); return 1; }
    int ok = fread(pixels, 1, size, in) == size && fgetc(in) == EOF;
    fclose(in);
    if (ok) ok = qoi_write(argv[3], pixels, &desc) != 0;
    free(pixels);
    return ok ? 0 : 1;
}
