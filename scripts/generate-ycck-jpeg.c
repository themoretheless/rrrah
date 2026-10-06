/* External fixture encoder only; libjpeg-turbo is not a production dependency. */
#include <stdio.h>
#include <jpeglib.h>
#include <string.h>
int main(int argc, char **argv) {
    if (argc != 2 && !(argc == 3 && strcmp(argv[2], "--asymmetric") == 0)) return 2;
    int asymmetric = argc == 3;
    FILE *out = fopen(argv[1], "wb"); if (!out) return 3;
    struct jpeg_compress_struct c; struct jpeg_error_mgr e;
    c.err = jpeg_std_error(&e); jpeg_create_compress(&c); jpeg_stdio_dest(&c, out);
    c.image_width = asymmetric ? 17 : 64; c.image_height = asymmetric ? 13 : 8; c.input_components = 4; c.in_color_space = JCS_CMYK;
    jpeg_set_defaults(&c); jpeg_set_colorspace(&c, JCS_YCCK); jpeg_set_quality(&c, 100, TRUE);
    for (int i = 0; i < c.num_components; ++i) { c.comp_info[i].h_samp_factor = 1; c.comp_info[i].v_samp_factor = 1; }
    jpeg_start_compress(&c, TRUE);
    const unsigned char colors[8][4] = {{0,0,0,0},{0,0,0,255},{255,0,0,0},{0,255,0,0},{0,0,255,0},{0,255,255,0},{255,0,255,0},{255,255,0,0}};
    unsigned char row[64*4];
    for (int x = 0; x < 64; ++x) for (int j = 0; j < 4; ++j) row[x*4+j] = colors[x/8][j];
    while (c.next_scanline < c.image_height) {
        if (asymmetric) for (unsigned int x = 0; x < c.image_width; ++x)
            for (int j = 0; j < 4; ++j)
                row[x*4+j] = (x*31 + c.next_scanline*47 + j*59) % 256;
        JSAMPROW rows[1] = {row}; jpeg_write_scanlines(&c, rows, 1); }
    jpeg_finish_compress(&c); jpeg_destroy_compress(&c); fclose(out); return 0;
}
