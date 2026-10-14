#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <stdbool.h>
static void *review_reallocarray(void *p,size_t n,size_t size){if(size && n>SIZE_MAX/size)return NULL;return realloc(p,n*size);}
#define reallocarray review_reallocarray
static inline char *dedup_colon_string(const char *value)
{
    if (!value || !*value) return strdup("");

    /* conservative first guess – avoids realloc in 99 % of cases */
    size_t  seg_cap = 32;
    char  **seg     = malloc(seg_cap * sizeof *seg);
    size_t  seg_cnt = 0;

    uint64_t seen_mask = 0;            /* bitmask for first 64 segments */
    size_t   out_len   = 1;            /* final '\0' */

    char *copy = strdup(value);
    for (char *tok = copy, *saveptr;; tok = NULL) {
        char *p = strtok_r(tok, ":", &saveptr);
        if (!p) break;

        /* linear search through the *unique* ones we already have */
        bool dup = false;
        for (size_t i = 0; i < seg_cnt; ++i) {
            if (strcmp(seg[i], p) == 0) { dup = true; break; }
        }
        if (dup) continue;

        /* grow array if needed */
        if (seg_cnt == seg_cap) {
            seg_cap *= 2;
            seg = reallocarray(seg, seg_cap, sizeof *seg);
        }
        seg[seg_cnt] = p;
        out_len += strlen(p) + 1;   /* +1 for ':' or '\0' */
        ++seg_cnt;
    }

    /* build result */
    char *out = malloc(out_len), *wp = out;
    for (size_t i = 0; i < seg_cnt; ++i) {
        size_t len = strlen(seg[i]);
        if (i) *wp++ = ':';
        memcpy(wp, seg[i], len);
        wp += len;
    }
    *wp = '\0';

    free(copy);          /* we did *not* strdup the individual segments */
    free(seg);
    return out;
}
int main(int argc,char **argv){char *p=dedup_colon_string(argc>1?argv[1]:NULL);if(!p)return 2;puts(p);free(p);return 0;}
