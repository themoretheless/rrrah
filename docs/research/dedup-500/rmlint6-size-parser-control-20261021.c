#include <stdbool.h>
#include <glib.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>
#include <math.h>
#include <errno.h>
#include <limits.h>
#include <stdio.h>
typedef guint64 RmOff;
#define RM_ERROR_QUARK (g_quark_from_static_string("rmlint"))
#define _(s) s
static const struct FormatSpec {
    const char *id;
    unsigned base;
    unsigned exponent;
} SIZE_FORMAT_TABLE[] = {
    /* This list is sorted, so bsearch() can be used */
    {.id = "b",  .base = 512,  .exponent = 1},
    {.id = "c",  .base = 1,    .exponent = 1},
    {.id = "e",  .base = 1000, .exponent = 6},
    {.id = "eb", .base = 1024, .exponent = 6},
    {.id = "g",  .base = 1000, .exponent = 3},
    {.id = "gb", .base = 1024, .exponent = 3},
    {.id = "k",  .base = 1000, .exponent = 1},
    {.id = "kb", .base = 1024, .exponent = 1},
    {.id = "m",  .base = 1000, .exponent = 2},
    {.id = "mb", .base = 1024, .exponent = 2},
    {.id = "p",  .base = 1000, .exponent = 5},
    {.id = "pb", .base = 1024, .exponent = 5},
    {.id = "t",  .base = 1000, .exponent = 4},
    {.id = "tb", .base = 1024, .exponent = 4},
    {.id = "w",  .base = 2,    .exponent = 1}
};
/* clang-format on */

typedef struct FormatSpec FormatSpec;

static const int SIZE_FORMAT_TABLE_N = sizeof(SIZE_FORMAT_TABLE) / sizeof(FormatSpec);

static int rm_util_compare_spec_elem(const void *fmt_a, const void *fmt_b) {
    return strcasecmp(((FormatSpec *)fmt_a)->id, ((FormatSpec *)fmt_b)->id);
}

guint64 rm_util_size_string_to_bytes(const char *size_spec, GError **error) {
    if(size_spec == NULL) {
        g_set_error(error, RM_ERROR_QUARK, 0, _("Input size is empty"));
        return 0;
    }

    // We have a small issue here:
    // 1) strold can not parse all of a guin64.
    // 2) strtoull can only parse integers.
    //
    // We "solve" this by parsing with strtoull until the dot,
    // and continue with strtold after. If we hit an overflow
    // during multiplication later we warn the user and abort.

    // Copy and strip the string:
    char size_spec_copy[512] = {0};
    memset(size_spec_copy, 0, sizeof(size_spec_copy));
    strncpy(size_spec_copy, size_spec, sizeof(size_spec_copy) - 1);
    char *size_spec_stripped = g_strstrip(size_spec_copy);

    if(*size_spec_stripped == 0) {
        g_set_error(error, RM_ERROR_QUARK, 0, _("Size is empty"));
        return 0;
    }

    if(*size_spec_stripped == '-') {
        g_set_error(error, RM_ERROR_QUARK, 0, _("Negative sizes are no good idea"));
        return 0;
    }

    // Actual number before the dot.
    RmOff base_size = 1;

    // size_factor to multiply the result with (in case of a suffix like »mb«)
    double size_factor = 1;

    // possible fraction given for a size after a dot.
    double fraction = 0;

    char *size_format = size_spec_stripped;
    for(; *size_format && (*size_format == '.' || g_ascii_isdigit(*size_format));
        size_format++)
        ;

    // Set to true when either a format suffix or a fraction was encountered.
    bool need_multiply = false;

    if(*size_format != 0) {
        need_multiply = true;
        FormatSpec key = {.id = size_format};
        const FormatSpec *found = bsearch(&key,
                                    SIZE_FORMAT_TABLE,
                                    SIZE_FORMAT_TABLE_N,
                                    sizeof(FormatSpec),
                                    rm_util_compare_spec_elem);

        if(found != NULL) {
            size_factor *= pow(found->base, found->exponent);
        } else {
            g_set_error(error, RM_ERROR_QUARK, 0,
                        _("Given size_format specifier not found"));
            return 0;
        }

        // Set to zero to separate it from the after-dot part.
        *size_format = 0;
    }

    char *dot = strchr(size_spec_stripped, '.');
    if(dot != NULL) {
        need_multiply = true;

        // Parse with strtoull since we only accept numbers after the dot.
        // (not 1e10 or negative number like strtod would support them)
        char *first_error = NULL;
        RmOff fraction_num = strtoull(&dot[1], &first_error, 10);
        if(first_error != NULL && *first_error != 0) {
            g_set_error(error, RM_ERROR_QUARK, 0, _("Failed to parse size fraction"));
            return 0;
        }

        if(fraction_num == ULLONG_MAX && errno == ERANGE) {
            g_set_error(error, RM_ERROR_QUARK, 0, _("Fraction is too big for uint64"));
            return 0;
        }

        // Make sure number parsing doesn't see the dot.
        *dot = 0;

        // Convert the integer (e.g. 523) to a fraction (e.g. 0.523).
        fraction = ((double)fraction_num) / pow(10, 1 + ((int)log10(fraction_num)));
    }

    if(*size_spec_stripped != 0) {
        char *first_error = NULL;
        base_size = strtoull(size_spec_stripped, &first_error, 10);
        if(first_error != NULL && *first_error != 0) {
            g_set_error(error, RM_ERROR_QUARK, 0, _("This does not look like a number"));
            return 0;
        }

        if(base_size == ULLONG_MAX && errno == ERANGE) {
            g_set_error(error, RM_ERROR_QUARK, 0, _("Size is too big for uint64"));
            return 0;
        }
    }

    RmOff result = base_size;
    if(need_multiply) {
        // Only multiply if we really have to.
        double fres = (result + fraction) * size_factor;

        /* Check if an overflow happened during multiplication.
         * NOTE: ULLONG_MAX is converted to (double)ULLONG_MAX+1, which is 0x1p64 */
        if(fres > nextafter((double)ULLONG_MAX, 0)) {
            g_set_error(error, RM_ERROR_QUARK, 0,
                        _("Size factor would overflow size (max. 2**64 allowed)"));
            return 0;
        }
        result = fres;
    }

    return result;
}

int main(void){const char *v[]={"1.5k","1.05k","1.005k","0","1kb"};unsigned long long expect[]={1500,1500,1500,0,1024};for(int i=0;i<5;i++){GError *e=NULL;errno=0;unsigned long long n=rm_util_size_string_to_bytes(v[i],&e);printf("%s=%llu error=%s\n",v[i],n,e?e->message:"none");if(e||n!=expect[i])return 1;}return 0;}
