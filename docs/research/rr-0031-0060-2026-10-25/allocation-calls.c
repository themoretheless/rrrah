/* Process-wide libc allocator call counts, not all allocations/peak live bytes.
 * Test instrumentation only; kernel peak RSS and managed budget are separate. */
#include <stdlib.h>
#include <stdio.h>
#include <stdatomic.h>
static _Atomic unsigned long long allocations, callocs, reallocations, alignments;
static void *count_malloc(size_t n){atomic_fetch_add(&allocations,1);return malloc(n);}
static void *count_calloc(size_t n,size_t s){atomic_fetch_add(&callocs,1);return calloc(n,s);}
static void *count_realloc(void *p,size_t n){atomic_fetch_add(&reallocations,1);return realloc(p,n);}
static int count_aligned(void **p,size_t a,size_t n){atomic_fetch_add(&alignments,1);return posix_memalign(p,a,n);}
#define INTERPOSE(replacement,original) __attribute__((used)) static struct {const void *replacement;const void *original;} interpose_##original __attribute__((section("__DATA,__interpose"))) = {(const void*)replacement,(const void*)original};
INTERPOSE(count_malloc,malloc)
INTERPOSE(count_calloc,calloc)
INTERPOSE(count_realloc,realloc)
INTERPOSE(count_aligned,posix_memalign)
__attribute__((destructor)) static void report(void){fprintf(stderr,"ALLOCATION_CALLS malloc=%llu calloc=%llu realloc=%llu aligned=%llu\n",atomic_load(&allocations),atomic_load(&callocs),atomic_load(&reallocations),atomic_load(&alignments));}
