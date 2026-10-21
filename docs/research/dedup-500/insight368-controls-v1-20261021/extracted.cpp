
//
// Copyright (c) 2009-2014 Joe Bertolami. All Right Reserved.
//
// vnInsight.h
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
// Copyright (c) 2009-2014 Joe Bertolami. Original function under BSD-2-Clause.
// License text retained in source-review evidence and original vnInsight.h.
#include <cstdint>
#include <cassert>
#include <iostream>
using UINT64=uint64_t;using UINT32=uint32_t;using FLOAT32=float;
#define CONST const
#define VN_INSIGHT_DEFAULT_HASH_SIZE 32
struct CVImage {UINT64 synthetic_hash;};
UINT64 vnHashImage64(CONST CVImage& image){return image.synthetic_hash;}
FLOAT32 vnCompareImages64( CONST CVImage & pA, CONST CVImage & pB )
{
    UINT64 uiHashA = vnHashImage64( pA );
    UINT64 uiHashB = vnHashImage64( pB );

    //
    // Compute the Hamming distance of our two hashes and return
    // the count as a representative of the degree of similarity.
    //

    UINT32  uiMatchCount  = 0;
    UINT64 uiCompareValue = uiHashA ^ uiHashB;

    while ( uiCompareValue )
    {
        uiCompareValue &= ( uiCompareValue - 1 );

        uiMatchCount++;
    }

    uiMatchCount = ( VN_INSIGHT_DEFAULT_HASH_SIZE << 3 ) - uiMatchCount;

    return ( uiMatchCount / ( (FLOAT32) ( VN_INSIGHT_DEFAULT_HASH_SIZE << 3 ) ) );
}


int main(){
 assert(vnCompareImages64({0},{0})==1.0f);
 std::cout<<"PASS identical synthetic hashes return1\n";
 assert(vnCompareImages64({0},{UINT64_MAX})==0.75f);
 std::cout<<"PASS opposite64bit hashes return0.75 (64of256 denominator)\n";
 assert(vnCompareImages64({0},{1})==255.0f/256.0f);
 std::cout<<"PASS one-bit change returns255/256 rather than63/64\n";
}
