#include "libphash.h"
#include <stdio.h>
#include <string.h>
int main(void) {
    const unsigned char a[]="P6\n2 1\n255\n\xff\0\0\0\xff\0";
    const unsigned char b[]="P6\n2 1\n255\n\0\xff\0\xff\0\0";
    ph_context_t *ctx=NULL;uint64_t ha=0,hb=0;ph_digest_t ma,mb;
    if(ph_create(&ctx)!=PH_SUCCESS)return 1;
    if(ph_load_from_memory(ctx,a,sizeof(a)-1)!=PH_SUCCESS)return 2;
    if(ph_compute_color_hash(ctx,&ha)!=PH_SUCCESS)return 3;
    if(ph_compute_color_moments_hash(ctx,&ma)!=PH_SUCCESS)return 4;
    if(ph_load_from_memory(ctx,b,sizeof(b)-1)!=PH_SUCCESS)return 5;
    if(ph_compute_color_hash(ctx,&hb)!=PH_SUCCESS)return 6;
    if(ph_compute_color_moments_hash(ctx,&mb)!=PH_SUCCESS)return 7;
    printf("different_spatial_pixels_colorhash_equal=%d moments_equal=%d moments_size=%u\n",ha==hb,ma.size==mb.size&&!memcmp(ma.data,mb.data,ma.size),ma.size);
    ph_free(ctx);
    return !(ha==hb&&ma.size==mb.size&&!memcmp(ma.data,mb.data,ma.size));
}
