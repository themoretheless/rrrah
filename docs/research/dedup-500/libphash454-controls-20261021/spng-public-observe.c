#include "internal.h"
int observed_first(ph_context_t *c) { return c->image.raw_rgb ? c->image.raw_rgb[0] : -1; }
