#include <lcms2.h>
#include <stdio.h>
int main(int argc,char **argv) {
 if(argc!=3)return 2;
 cmsHPROFILE source=cmsOpenProfileFromFile(argv[1],"r");
 cmsCIExyY white={0.3127,0.3290,1.0};
 cmsCIExyYTRIPLE primaries={{0.64,0.33,1.0},{0.30,0.60,1.0},{0.15,0.06,1.0}};
 cmsToneCurve *curves[3];for(int i=0;i<3;i++)curves[i]=cmsBuildGamma(NULL,1.0);
 cmsHPROFILE target=cmsCreateRGBProfile(&white,&primaries,curves);
 if(!source||!target)return 3;
 cmsHTRANSFORM transform=cmsCreateTransform(source,TYPE_GRAY_DBL,target,TYPE_RGB_DBL,INTENT_RELATIVE_COLORIMETRIC,cmsFLAGS_NOOPTIMIZE);
 if(!transform)return 4;
 double input[65536],output[65536*3];
 for(int i=0;i<65536;i++)input[i]=i/65535.;
 cmsDoTransform(transform,input,output,65536);
 FILE *file=fopen(argv[2],"wb");if(!file)return 5;
 if(fwrite(output,sizeof(double),65536*3,file)!=65536*3)return 6;
 fclose(file);cmsDeleteTransform(transform);cmsCloseProfile(source);cmsCloseProfile(target);
 for(int i=0;i<3;i++)cmsFreeToneCurve(curves[i]);
 printf("LittleCMS %u: 65536 gray inputs, relative colorimetric, double linear sRGB output\n",cmsGetEncodedCMMversion());
 return 0;
}
