/* Test-only independent LittleCMS RGB16 -> linear sRGB oracle. */
#include <lcms2.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
int main(int argc, char **argv) {
  if (argc != 4) return 2;
  FILE *input = fopen(argv[2], "rb");
  if (!input) return 3;
  uint8_t bytes[200];
  if (fread(bytes, 1, sizeof(bytes), input) != sizeof(bytes) || fgetc(input) != EOF) return 4;
  fclose(input);
  double rgb[75], alpha[25], transformed[75];
  for (int p = 0; p < 25; ++p) for (int c = 0; c < 4; ++c) {
    int at = (p * 4 + c) * 2;
    double sample = (bytes[at] | ((uint16_t) bytes[at + 1] << 8)) / 65535.0;
    if (c == 3) alpha[p] = sample; else rgb[p * 3 + c] = sample;
  }
  cmsHPROFILE source = cmsOpenProfileFromFile(argv[1], "r");
  cmsCIExyY white = {0.3127, 0.3290, 1.0};
  cmsCIExyYTRIPLE primaries = {{0.64, 0.33, 1.0}, {0.30, 0.60, 1.0}, {0.15, 0.06, 1.0}};
  cmsToneCurve *curves[3];
  for (int c = 0; c < 3; ++c) curves[c] = cmsBuildGamma(NULL, 1.0);
  cmsHPROFILE target = cmsCreateRGBProfile(&white, &primaries, curves);
  if (!source || !target) return 5;
  cmsHTRANSFORM transform = cmsCreateTransform(source, TYPE_RGB_DBL, target, TYPE_RGB_DBL,
      INTENT_RELATIVE_COLORIMETRIC, cmsFLAGS_NOOPTIMIZE);
  if (!transform) return 6;
  cmsDoTransform(transform, rgb, transformed, 25);
  FILE *output = fopen(argv[3], "wb");
  if (!output) return 7;
  for (int p = 0; p < 25; ++p) for (int c = 0; c < 4; ++c) {
    double value = c == 3 ? alpha[p] : transformed[p * 3 + c];
    uint64_t bits; memcpy(&bits, &value, 8);
    for (int b = 0; b < 8; ++b) if (fputc((int)((bits >> (b * 8)) & 255), output) == EOF) return 8;
  }
  if (fclose(output)) return 9;
  cmsDeleteTransform(transform); cmsCloseProfile(source); cmsCloseProfile(target);
  for (int c = 0; c < 3; ++c) cmsFreeToneCurve(curves[c]);
  printf("LittleCMS %u, RGB_DBL->RGB_DBL, relative-colorimetric, NOOPTIMIZE, 25 pixels\n", cmsGetEncodedCMMversion());
  return 0;
}
