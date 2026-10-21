from pathlib import Path
import sys,subprocess
root=Path(sys.argv[1]);h=(root/'src/cvutil.h').read_text();s=(root/'src/cvutil.cpp').read_text()
structs=h[h.index('struct DescriptorColor {'):h.index('// conversions to/from CImg')]
method=s[s.index('float ColorDescriptor::distance('):s.index('// greys are not colors technically')]
shim='#include <cstdint>\n#include <cstring>\n#include <cmath>\n#include <cfloat>\n#include <cassert>\n#include <cstdio>\n#include <cstdlib>\nclass QColor;namespace cv {class Mat;}\n#define USE_EMD 0\n'
tests=r'''
int main(){ColorDescriptor a,b;a.numColors=b.numColors=2;a.colors[0].l=0;a.colors[1].l=10000;b.colors[0].l=0;b.colors[1].l=60000;float ab=ColorDescriptor::distance(a,b),ba=ColorDescriptor::distance(b,a);assert(fabsf(ab-ba)>1);puts("{\"name\":\"equal-count-color-distance-is-asymmetric\",\"passed\":true}");assert(ColorDescriptor::distance(a,a)==1);puts("{\"name\":\"identical-descriptor-baseline-is-one\",\"passed\":true}");auto c=a;c.colors[0].w=1;c.colors[1].w=65535;a.colors[0].w=65535;a.colors[1].w=1;assert(ColorDescriptor::distance(a,c)==1);puts("{\"name\":\"frequency-weight-changes-do-not-change-score\",\"passed\":true}");ColorDescriptor empty;assert(ColorDescriptor::distance(a,empty)==FLT_MAX);puts("{\"name\":\"empty-descriptor-is-not-comparable\",\"passed\":true}");}
'''
p=root/'color-distance-controls.cpp';p.write_text(shim+structs+method+tests);binary=root/'color-distance-controls';subprocess.run(['clang++','-std=c++17','-O2',str(p),'-o',str(binary)],check=True);subprocess.run([str(binary.resolve())],check=True)
