
//
// Copyright (c) 2002-2014 Joe Bertolami. All Right Reserved.
//
// vnPlatform.h
//
//   Redistribution and use in source and binary forms, with or without
//   modification, are permitted provided that the following conditions are met:
//
//   * Redistributions of source code must retain the above copyright notice, this
//     list of conditions and the following disclaimer.
//
//   * Redistributions in binary form must reproduce the above copyright notice,
//     this list of conditions and the following disclaimer in the documentation
//     and/or other materials provided with the distribution.
//
//   THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
//   AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
//   IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
//   DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
//   FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
//   DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
//   SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
//   CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
//   OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
//   OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//
// Authored type/error adapters; original function body follows.
#include <cstdint>
#include <cassert>
#include <iostream>
using UINT8=uint8_t;using UINT32=uint32_t;using INT16=int16_t;using VN_STATUS=int;
#define IN
#define VN_PARAM_CHECK 1
#define VN_ERROR_INVALIDARG -1
#define VN_SUCCESS 0
int vnPostError(int x){return x;}
VN_STATUS vnDesaturateLine( IN UINT8 * pInput, UINT32 uiPixelCount, UINT8 * pOutput )
{
    if ( VN_PARAM_CHECK )
    {
        if ( !pInput || 0 == uiPixelCount || !pOutput )
        {
            return vnPostError( VN_ERROR_INVALIDARG );
        }
    }

    //
    // We generate a grayscale image through desaturation. This method is preferred because
    // it favors the color channels in a human perceptual manner.
    //

    for ( UINT32 iX = 0; iX < uiPixelCount; iX++ )
    {
        UINT8 * pSrcPixel   = pInput + iX * 3;
        UINT8 * pDestPixel  = pOutput + iX;

        INT16 iSrcRed   = pSrcPixel[0];
        INT16 iSrcGreen = pSrcPixel[1];
        INT16 iSrcBlue  = pSrcPixel[2];

        pDestPixel[0] = ( ( iSrcRed * 0x4CC ) + ( iSrcGreen * 0x970 ) + ( iSrcBlue * 0x1C2 ) ) >> 12;
    }

    return VN_SUCCESS;
}


int main(){
 for(unsigned v=0;v<256;v++){
  UINT8 rgb[3]={UINT8(v),UINT8(v),UINT8(v)}, out=0;
  assert(vnDesaturateLine(rgb,1,&out)==0);
  assert(out==(v==0?0:v-1));
 }
 std::cout<<"PASS all256 neutral levels:0 maps0;1..255 each reduced by1\n";
 UINT8 rgb[3]={255,255,255},out=0;
 assert(vnDesaturateLine(rgb,1,&out)==0 && out==254);
 std::cout<<"PASS opaque white converts254 rather than255\n";
 assert(vnDesaturateLine(nullptr,1,&out)==-1);
 assert(vnDesaturateLine(rgb,0,&out)==-1);
 assert(vnDesaturateLine(rgb,1,nullptr)==-1);
 std::cout<<"PASS null pointers and zero pixel count explicitly refused\n";
}
