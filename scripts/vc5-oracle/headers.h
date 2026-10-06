/* Minimal host adapter for unmodified official GoPro vlc.c/vlc.h. Oracle only. */
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <assert.h>
typedef uint32_t BITWORD;
typedef uint32_t BITCOUNT;
typedef int CODEC_ERROR;
#define CODEC_ERROR_OKAY 0
#define CODEC_ERROR_NOTFOUND 1
#define neg(x) (-(x))
typedef struct { const uint8_t *bytes; size_t length, bit; } BITSTREAM;
static BITWORD GetBits(BITSTREAM *s, BITCOUNT n) {
    BITWORD v = 0;
    for (BITCOUNT i = 0; i < n; ++i) {
        if (s->bit / 8 >= s->length) { fprintf(stderr,"truncated oracle input\n"); exit(7); }
        v = (v << 1) | ((s->bytes[s->bit/8] >> (7-s->bit%8)) & 1);
        ++s->bit;
    }
    return v;
}
static BITWORD AddBits(BITSTREAM *s, BITWORD v, BITCOUNT n) { return (v << n) | GetBits(s,n); }
#include "vlc.h"
#include <string.h>
#define STATIC_INLINE static inline
#define absolute(x) abs(x)
#include "pixel.h"
int32_t UncompandedValue(int32_t value);
