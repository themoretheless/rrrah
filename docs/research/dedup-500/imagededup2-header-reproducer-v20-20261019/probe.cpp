#include "builtin/builtin.h"
PSNIP_BUILTIN__POPCOUNT_DEFINE_PORTABLE(review_popcountll, unsigned long long)
extern "C" int selected_count(unsigned long long x) { return psnip_builtin_popcountll(x); }
extern "C" int portable_count(unsigned long long x) { return psnip_builtin_review_popcountll(x); }
