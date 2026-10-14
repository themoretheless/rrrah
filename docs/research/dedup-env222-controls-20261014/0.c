#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <stdbool.h>
static void *review_reallocarray(void *p,size_t n,size_t size){if(size && n>SIZE_MAX/size)return NULL;return realloc(p,n*size);}
#define reallocarray review_reallocarray
static char *dedup_var_value(const char *value) {
    if (!value || !*value) return strdup("");

    char **segments = NULL;
    char *copy = strdup(value);
    int segment_count = 0;
    char *token, *rest = copy;

    // Split into segments
    while ((token = strtok_r(rest, ":", &rest))) {
        segments = realloc(segments, (segment_count + 1) * sizeof(char *));
        segments[segment_count++] = strdup(token);
    }

    // Mark duplicates (keep first occurrence)
    int *keep = calloc(segment_count, sizeof(int));
    for (int i = 0; i < segment_count; i++) {
        keep[i] = 1;
        for (int j = 0; j < i; j++) {
            if (strcmp(segments[j], segments[i]) == 0) {
                keep[i] = 0;
                break;
            }
        }
    }

    // Calculate required size
    size_t total_len = 1; // Null terminator
    for (int i = 0; i < segment_count; i++) {
        if (keep[i]) total_len += strlen(segments[i]) + 1; // +1 for colon
    }

    // Build result
    char *result = malloc(total_len);
    result[0] = '\0';
    int first = 1;
    for (int i = 0; i < segment_count; i++) {
        if (keep[i]) {
            if (!first) strcat(result, ":");
            strcat(result, segments[i]);
            first = 0;
        }
    }

    // Cleanup
    for (int i = 0; i < segment_count; i++) free(segments[i]);
    free(segments);
    free(keep);
    free(copy);

    return result;
}
int main(int argc,char **argv){char *p=dedup_var_value(argc>1?argv[1]:NULL);if(!p)return 2;puts(p);free(p);return 0;}
