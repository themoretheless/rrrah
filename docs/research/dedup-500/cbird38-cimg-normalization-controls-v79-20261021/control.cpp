// Research control for the unmodified, pinned CImg header. No file decoder,
// Qt qualityScore, or pHash implementation is executed here.
#include "CImg.h"
#include <cstdio>
#include <cstdlib>
using cimg_library::CImg;

static void require(bool value, const char* label) {
  if (!value) {
    std::fprintf(stderr, "FAIL: %s\n", label);
    std::exit(1);
  }
}

int main() {
  const CImg<float> dark(2, 2, 1, 1, 0), light(2, 2, 1, 1, 137);
  const auto normalized_dark = dark.get_normalize(0, 255);
  const auto normalized_light = light.get_normalize(0, 255);
  for (unsigned long i = 0; i < dark.size(); ++i) {
    require(normalized_dark[i] == 0 && normalized_light[i] == 0,
            "constant inputs collapse to lower endpoint by default");
    require(dark[i] == 0 && light[i] == 137, "copy normalization preserves inputs");
  }
  require(light.get_normalize(0, 255, 1)(0) == 255, "explicit constant ratio selects upper endpoint");
  std::puts("PASS 1: default constant normalization collapses distinct inputs; copy API retains originals");

  CImg<float> red(1, 1, 1, 3, 0), green(1, 1, 1, 3, 0);
  red(0, 0, 0, 0) = 1;
  green(0, 0, 0, 1) = 1;
  require(red.get_norm(1)(0) == 1 && green.get_norm(1)(0) == 1,
          "L1 norm does not retain RGB component identity");
  CImg<float> signed_pixel(1, 1, 1, 1, -3);
  require(signed_pixel.get_norm(1)(0) == 3, "single-channel norm uses absolute value");
  std::puts("PASS 2: L1 norm is an absolute channel sum, not color identity or weighted luminance");
}
