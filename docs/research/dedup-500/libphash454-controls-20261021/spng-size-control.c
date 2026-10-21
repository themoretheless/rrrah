#include "spng.c"
int observed_sizes(unsigned color, unsigned depth, uint32_t width, uint32_t height,
                   size_t *scanline, size_t *rgb_size) {
    struct spng_ihdr header = {width, height, depth, color, 0, 0, 0};
    int error = calculate_scanline_width(&header, width, scanline);
    if (error) return error;
    return calculate_image_size(&header, SPNG_FMT_RGB8, rgb_size);
}
