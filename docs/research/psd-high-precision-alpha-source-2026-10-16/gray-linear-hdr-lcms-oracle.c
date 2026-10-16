#include <lcms2.h>
#include <stdio.h>
#include <stdint.h>
#include <string.h>
int main(int argc,char **argv) {
 cmsHPROFILE source=cmsOpenProfileFromFile(argv[1],"r");
 cmsCIExyY white={0.3127,0.3290,1};
 cmsCIExyYTRIPLE primaries={{.64,.33,1},{.30,.60,1},{.15,.06,1}};
 cmsToneCurve *curves[3];for(int c=0;c<3;c++)curves[c]=cmsBuildGamma(NULL,1);
 cmsHPROFILE target=cmsCreateRGBProfile(&white,&primaries,curves);
 cmsHTRANSFORM transform=cmsCreateTransform(source,TYPE_GRAY_DBL,target,TYPE_RGB_DBL,INTENT_RELATIVE_COLORIMETRIC,cmsFLAGS_NOOPTIMIZE);
 if(!transform)return 2;
 float input32[4]={1.00001f,2.f,4.f,16.f};double input[4],output[12];for(int c=0;c<4;c++)input[c]=input32[c];
 cmsDoTransform(transform,input,output,4);
 FILE *f=fopen(argv[2],"wb");if(!f)return 3;
 for(int c=0;c<12;c++){uint64_t bits;memcpy(&bits,&output[c],8);for(int j=0;j<8;j++)fputc((bits>>(j*8))&255,f);printf("%.15g\n",output[c]);}
 return fclose(f)!=0;
}
