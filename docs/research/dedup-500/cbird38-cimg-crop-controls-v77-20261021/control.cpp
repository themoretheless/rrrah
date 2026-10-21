#include "CImg.h"
#include <cstdio>
#include <cstdlib>
using cimg_library::CImg;
static void require(bool ok, const char* what) {
  if (!ok) { std::fprintf(stderr, "FAIL: %s\n", what); std::exit(1); }
}
static void quality_crop(CImg<unsigned char>& src) {
  const int hCrop = int(src.width() * 0.10);
  const int vCrop = int(src.height() * 0.10);
  src.crop(hCrop, vCrop, 0, 0, src.width() - hCrop, src.height() - vCrop, 0, 0);
}
int main() {
  CImg<unsigned char> a(100,100,1,3,0), b(100,100,1,3,0);
  for (int y=0;y<100;++y) for (int x=0;x<100;++x) {
    a(x,y,0,0)=b(x,y,0,0)=37;
    a(x,y,0,1)=10; a(x,y,0,2)=20;
    b(x,y,0,1)=210; b(x,y,0,2)=240;
  }
  quality_crop(a); quality_crop(b);
  require(a.spectrum()==1 && b.spectrum()==1, "explicit channel crop retains only channel0");
  auto ga=a.get_norm(1), gb=b.get_norm(1);
  for (unsigned long i=0;i<ga.size();++i) require(ga[i]==gb[i] && ga[i]==37, "green/blue differences vanish before quality filtering");
  std::puts("PASS 1: RGB differences in green/blue removed; resulting norm inputs identical");
  require(a.width()==81 && a.height()==81, "inclusive crop endpoints produce81, not80");
  std::puts("PASS 2:100x100 ten-percent crop yields81x81");
  CImg<unsigned char> small(9,9,1,3,77); quality_crop(small);
  require(small.width()==10 && small.height()==10 && small.spectrum()==1, "zero-margin crop grows input");
  require(small(8,8)==77 && small(9,8)==0 && small(8,9)==0, "out-of-range row/column are zero padded");
  std::puts("PASS 3:9x9 zero-margin crop yields10x10 with zero right/bottom padding");
}
