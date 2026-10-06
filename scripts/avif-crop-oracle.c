// External qualification only. Compile with matching libavif headers/library.
// Production rrrah does not link this tool or libavif.
#include <avif/avif.h>
#include <stdio.h>
#include <string.h>

int main(int argc, char **argv) {
    if (argc < 2) return 2;
    fprintf(stderr, "libavif %s\n", avifVersion());
    if (argc == 4 && strcmp(argv[1], "--export-top-left") == 0) {
        avifDecoder *decoder = avifDecoderCreate();
        avifImage *image = avifImageCreateEmpty();
        avifEncoder *encoder = avifEncoderCreate();
        avifRWData encoded = AVIF_DATA_EMPTY;
        avifDiagnostics diagnostics = {0};
        if (!decoder || !image || !encoder) return 3;
        avifResult result = avifDecoderReadFile(decoder, image, argv[2]);
        avifCropRect crop = {0, 0, image->width / 2, image->height / 2};
        if (result == AVIF_RESULT_OK &&
            avifCleanApertureBoxFromCropRect(&image->clap, &crop, image->width,
                                            image->height, &diagnostics)) {
            image->transformFlags |= AVIF_TRANSFORM_CLAP;
            encoder->speed = AVIF_SPEED_FASTEST;
            encoder->quality = AVIF_QUALITY_LOSSLESS;
            encoder->qualityAlpha = AVIF_QUALITY_LOSSLESS;
            result = avifEncoderWrite(encoder, image, &encoded);
        } else {
            result = AVIF_RESULT_INVALID_ARGUMENT;
        }
        int status = 1;
        if (result == AVIF_RESULT_OK) {
            FILE *file = fopen(argv[3], "wb");
            if (file) {
                status = fwrite(encoded.data, 1, encoded.size, file) == encoded.size ? 0 : 1;
                if (fclose(file) != 0) status = 1;
            }
        }
        if (status) fprintf(stderr, "%s; %s\n", avifResultToString(result), diagnostics.error);
        avifRWDataFree(&encoded);
        avifEncoderDestroy(encoder);
        avifImageDestroy(image);
        avifDecoderDestroy(decoder);
        return status;
    }
    for (int i = 1; i < argc; ++i) {
        avifDecoder *decoder = avifDecoderCreate();
        avifImage *image = avifImageCreateEmpty();
        if (!decoder || !image) return 3;
        avifResult result = avifDecoderReadFile(decoder, image, argv[i]);
        avifCropRect crop = {0};
        avifDiagnostics diagnostics = {0};
        if (result != AVIF_RESULT_OK || !(image->transformFlags & AVIF_TRANSFORM_CLAP) ||
            !avifCropRectFromCleanApertureBox(&crop, &image->clap, image->width,
                                            image->height, &diagnostics)) {
            fprintf(stderr, "%s: %s; %s\n", argv[i], avifResultToString(result), diagnostics.error);
            avifImageDestroy(image);
            avifDecoderDestroy(decoder);
            return 1;
        }
        printf("%s\t%u\t%u\t%u\t%u\t%u\t%u\t%u\t%u\t%u\n", argv[i],
               image->width, image->height, crop.x, crop.y, crop.width, crop.height,
               image->transformFlags, image->irot.angle, image->imir.axis);
        avifImageDestroy(image);
        avifDecoderDestroy(decoder);
    }
    return 0;
}
