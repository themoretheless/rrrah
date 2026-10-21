from pathlib import Path
import subprocess,sys
root=Path(sys.argv[1]);s=(root/'src/media.cpp').read_text();method=s[s.index('QImage Media::constrainedResize('):s.index('void Media::readMetadata(')]
shim=r'''
#include <cassert>
#include <cstdio>
struct QSize{int w,h;QSize(int w=0,int h=0):w(w),h(h){}int width()const{return w;}int height()const{return h;}bool operator!=(QSize x)const{return w!=x.w||h!=x.h;}};
namespace Qt{enum{IgnoreAspectRatio,SmoothTransformation};}
struct QImage{int w,h;QImage(int w=0,int h=0):w(w),h(h){}bool isNull()const{return w<=0||h<=0;}int width()const{return w;}int height()const{return h;}QSize size()const{return {w,h};}QImage scaled(QSize s,int,int)const{return {s.w,s.h};}};
struct Media{static QImage constrainedResize(const QImage&,const QSize&);};
'''
tests=r'''
int main(){auto a=Media::constrainedResize({400,200},{100,0});assert(a.w==100&&a.h==50);puts("{\"name\":\"single-axis-aspect-ratio-ordinary-size\",\"passed\":true}");auto b=Media::constrainedResize({1,1000},{0,1});assert(b.w==0&&b.h==1);puts("{\"name\":\"thin-image-inferred-width-rounds-to-zero\",\"passed\":true}");auto c=Media::constrainedResize({400,200},{100,100});assert(c.w==100&&c.h==100);puts("{\"name\":\"two-positive-axes-request-ignore-aspect-ratio\",\"passed\":true}");}
'''
p=root/'resize-controls.cpp';p.write_text(shim+method+tests);binary=root/'resize-controls';subprocess.run(['clang++','-std=c++17','-O2',str(p),'-o',str(binary)],check=True);subprocess.run([str(binary.resolve())],check=True)
