#include "internal.h"
#include <stdio.h>
#include <stdlib.h>
int main(void) {
    static const unsigned char ppm[] = "P6\n1 1\n255\n\xff\0\0";
    ph_context_t *ctx = NULL;
    if (ph_create(&ctx) != PH_SUCCESS) return 1;
    if (ph_load_from_memory(ctx, ppm, sizeof(ppm)-1) != PH_SUCCESS) return 2;
    ph_context_set_gray_weights(ctx,128,0,0);
    unsigned custom = ph_get_gray(ctx)[0];
    ph_context_set_gray_weights(ctx,0,0,0);
    unsigned cached = ph_get_gray(ctx)[0];
    unsigned char fresh = 0;
    ph_to_grayscale(ctx,ctx->image.raw_rgb,1,1,3,&fresh);
    printf("custom_gray=%u reset_cached=%u reset_recomputed=%u\n",custom,cached,fresh);
    if (custom != 255 || cached != 255 || fresh != 75) return 3;
    const unsigned char junk[] = {0,0,0,0};
    int error = ph_load_from_memory(ctx,junk,sizeof(junk));
    printf("failed_reload_error=%d public_loaded=%d internal_loaded=%d\n",error,ph_is_loaded(ctx),ctx->image.is_loaded);
    if (error != PH_ERR_DECODE_FAILED || ph_is_loaded(ctx)!=0 || ctx->image.is_loaded!=1) return 4;
    ph_free(ctx);
    return 0;
}
