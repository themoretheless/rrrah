#include "spng.h"
#include <stdio.h>
#include <string.h>
int main(void) {
    spng_ctx *ctx = spng_ctx_new(SPNG_CTX_ENCODER);
    if (!ctx) return 1;
    struct spng_unknown_chunk input[2] = {
        {{'a','b','C','a'}, 0, NULL, SPNG_AFTER_IHDR},
        {{'d','e','F','b'}, 0, NULL, SPNG_AFTER_IDAT}
    }, output[2] = {0};
    uint32_t count = 0;
    if (spng_set_unknown_chunks(ctx,input,2)) return 2;
    if (spng_get_unknown_chunks(ctx,NULL,&count) || count != 2) return 3;
    if (spng_get_unknown_chunks(ctx,output,&count)) return 4;
    int first = memcmp(input[0].type,output[0].type,4)==0;
    int second = memcmp(input[1].type,output[1].type,4)==0;
    printf("reported_count=%u first_copied=%d second_copied=%d\n", count,first,second);
    spng_ctx_free(ctx);
    return first && !second ? 0 : 5;
}
