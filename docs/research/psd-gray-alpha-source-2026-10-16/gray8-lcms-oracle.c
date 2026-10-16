/* Test-only LittleCMS full-u16 grayscale -> double linear sRGB oracle. */
#include <lcms2.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
int main(int argc,char **argv) {
 if(argc!=3)return 2;
 cmsHPROFILE source=cmsOpenProfileFromFile(argv[1],"r");
 cmsCIExyY white={0.3127,0.3290,1.0};
 cmsCIExyYTRIPLE primaries={{0.64,0.33,1.0},{0.30,0.60,1.0},{0.15,0.06,1.0}};
 cmsToneCurve *curves[3];for(int i=0;i<3;i++)curves[i]=cmsBuildGamma(NULL,1.0);
 cmsHPROFILE target=cmsCreateRGBProfile(&white,&primaries,curves);
 if(!source||!target)return 3;
 cmsHTRANSFORM transform=cmsCreateTransform(source,TYPE_GRAY_DBL,target,TYPE_RGB_DBL,
   INTENT_RELATIVE_COLORIMETRIC,cmsFLAGS_NOOPTIMIZE);
 if(!transform)return 4;
 double *input=malloc(256*sizeof(double)),*output=malloc(256*3*sizeof(double));
 if(!input||!output)return 5;
 for(unsigned i=0;i<256;i++)input[i]=i/255.0;
 cmsDoTransform(transform,input,output,256);
 FILE *file=fopen(argv[2],"wb");if(!file)return 6;
 for(unsigned i=0;i<256*3;i++) {
  uint64_t bits;memcpy(&bits,&output[i],8);
  for(int j=0;j<8;j++)if(fputc((int)((bits>>(j*8))&255),file)==EOF)return 7;
 }
 if(fclose(file))return 8;
 free(input);free(output);cmsDeleteTransform(transform);cmsCloseProfile(source);cmsCloseProfile(target);
 for(int i=0;i<3;i++)cmsFreeToneCurve(curves[i]);
 printf("LittleCMS %u:256 Gray_DBL -> RGB_DBL, relative, NOOPTIMIZE, little-endian double output\n",cmsGetEncodedCMMversion());
 return 0;
}
