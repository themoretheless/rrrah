#include "spng.c"
int observed_defilter(const unsigned char *previous, const unsigned char *encoded,
                      unsigned char *out, size_t length, unsigned bpp, unsigned filter) {
    unsigned char *row = calloc(length + 64, 1), *prev = calloc(length + 64, 1);
    if (!row || !prev) { free(row); free(prev); return -100; }
    memcpy(row, encoded, length); memcpy(prev, previous, length);
    int error = defilter_scanline(prev, row, length + 1, bpp, filter);
    if (!error) memcpy(out, row, length);
    free(row); free(prev); return error;
}
