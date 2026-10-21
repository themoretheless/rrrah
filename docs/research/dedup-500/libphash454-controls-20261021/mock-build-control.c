#include "libphash.h"
#include <stdio.h>
int main(void) {
    ph_context_t *ctx=NULL;
    const unsigned char bytes[]={0xDE,0xAD,0xBE,0xEF};
    if(ph_create(&ctx)!=PH_SUCCESS)return 1;
    int result=ph_load_from_memory(ctx,bytes,sizeof(bytes));
    printf("invalid_four_byte_input_result=%d loaded=%d\n",result,ph_is_loaded(ctx));
    ph_free(ctx);
    return 0;
}
