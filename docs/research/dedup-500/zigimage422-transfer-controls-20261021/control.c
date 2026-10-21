#include <stdio.h>
#include <assert.h>
#include <math.h>
#define STB_IMAGE_RESIZE_IMPLEMENTATION
#include "stb_image_resize.h"
int main(void){
 double max_error=0;
 for(int i=0;i<256;i++){
  const double s=i/255.0;
  const double expected=s<=0.04045?s/12.92:pow((s+0.055)/1.055,2.4);
  double error=fabs(stbir__srgb_uchar_to_linear_float[i]-expected);
  if(error>max_error)max_error=error;
  assert(error<0.000001);
  if(i)assert(stbir__srgb_uchar_to_linear_float[i]>stbir__srgb_uchar_to_linear_float[i-1]);
  assert(stbir__linear_to_srgb_uchar(stbir__srgb_uchar_to_linear_float[i])==i);
 }
 assert(stbir__linear_to_srgb_uchar(NAN)==0);
 assert(stbir__linear_to_srgb_uchar(-INFINITY)==0);
 assert(stbir__linear_to_srgb_uchar(INFINITY)==255);
 printf("{\"srgb_values\":256,\"strictly_increasing\":true,\"roundtrips\":256,\"max_decode_error\":%.12g,\"nan\":0,\"negative_infinity\":0,\"positive_infinity\":255}\n",max_error);
}
