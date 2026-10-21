// Authored platform/memory/alignment adapters; original class and methods follow.
#include <cstdint>
#include <cstring>
#include <cassert>
#include <iostream>
using UINT8=uint8_t;using UINT32=uint32_t;using VN_STATUS=int;using BOOL=bool;
#define VOID void
#define CONST const
#define IN
#define INOUT
#define VN_PARAM_CHECK 1
#define VN_SUCCESS 0
#define VN_ERROR_INVALIDARG -1
#define VN_ERROR_EXECUTION_FAILURE -2
#define VN_ERROR_OUTOFMEMORY -3
#define VN_ERROR_CAPACITY_LIMIT -4
#define VN_ERROR_INVALID_RESOURCE -5
#define VN_FAILED(x) ((x)!=0)
#define VN_ERR(...) ((void)0)
#define VN_WRN(...) ((void)0)
#define VN_MIN2(a,b) ((a)<(b)?(a):(b))
#define TRUE true
#define FALSE false
int vnPostError(int x){return x;}
UINT32 vnAlign(UINT32 x,UINT32 a){return (x+a-1)/a*a;}
void vnCopyMemory(void*d,const void*s,UINT32 n){std::memcpy(d,s,n);}

//
// Copyright (c) 2002-2014 Joe Bertolami. All Right Reserved.
//
// vnBitStream.h
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
// Description:
//
//   This module is part of the Vision Basecode and has been compacted and reduced
//   for inclusion within Insight.
//
//  Additional Information:
//
//   For more information, visit http://www.bertolami.com.
//

#ifndef __VN_BIT_STREAM_H__
#define __VN_BIT_STREAM_H__


//
// Note that the BitStream is sized along byte boundaries, but
// reading and writing of bits is along bit boundaries.
//

class CVBitStream
{
	UINT32		m_uiReadIndex;			// bit index
	UINT32		m_uiWriteIndex;			// bit index
	UINT8 *	    m_DataStore;
    UINT32      m_DataCapacity;

protected:

	//
	// Copy methods return the number of bits written/read.
	//
	// Note that it is the caller's job to ensure the safety of the operation.
	//

	UINT32 AlignedCopy( UINT8 * pbyDest, UINT32 uiDestBitOffset, UINT8 * pbySource, UINT32 uiSourceBitOffset, UINT32 uiBitCountToCopy );
	UINT32 UnalignedCopy( UINT8 * pbyDest, UINT32 uiDestBitOffset, UINT8 * pbySource, UINT32 uiSourceBitOffset, UINT32 uiBitCountToCopy );

public:

    CVBitStream();
    CVBitStream( CONST CVBitStream & rvalue );
	CVBitStream( IN VOID * pbyBytes, UINT32 uiSize );
    virtual ~CVBitStream();

	//
	// Our size interfaces accept and return lengths in bits (not bytes!)
	//

    virtual UINT8 *          QueryData() CONST;
	virtual UINT32           QueryCapacity() CONST;
    virtual UINT32			 QueryOccupancy() CONST;
    virtual UINT32			 ResizeCapacity( UINT32 uiNewSizeInBits );

	//
	// Seek will adjust our read index. There is deliberately no way to
	// adjust the write index.
	//

	virtual VN_STATUS		 Seek( UINT32 uiBitOffset );
	virtual VN_STATUS		 Assign( CONST CVBitStream & rvalue );
    virtual VN_STATUS		 Assign( IN VOID * pbyBytes, UINT32 uiSize );
    virtual VN_STATUS		 Clear();
    virtual VN_STATUS		 Empty();

	virtual BOOL			 IsEmpty() CONST;
    virtual BOOL			 IsFull() CONST;

    virtual VN_STATUS        WriteByte( UINT8 uiValue );
	virtual VN_STATUS		 WriteBit( UINT8 uiValue );
    virtual VN_STATUS        WriteBytes( VOID * pData, UINT32 uiByteCount );
    virtual VN_STATUS		 WriteBits( VOID * pData, UINT32 uiBitCount );

    virtual VN_STATUS        ReadByte( VOID * pData );
	virtual VN_STATUS		 ReadBit( VOID * pData );
    virtual VN_STATUS        ReadBytes( VOID * pData, INOUT UINT32 * puiByteCount );
    virtual VN_STATUS		 ReadBits( VOID * pData, INOUT UINT32 * puiBitCount );

	virtual CVBitStream &	 operator = ( CONST CVBitStream & rvalue );
    virtual BOOL             operator == ( CONST CVBitStream & rvalue ) CONST;
    virtual BOOL             operator != ( CONST CVBitStream & rvalue ) CONST;
};

#endif // __VN_BIT_STREAM_H__


CVBitStream::CVBitStream()
{
	m_uiReadIndex  = 0;
	m_uiWriteIndex = 0;
    m_DataStore    = 0;
    m_DataCapacity = 0;
}

CVBitStream::CVBitStream( CONST CVBitStream & rvalue )
{
	if ( VN_FAILED( Assign( rvalue ) ) )
	{
		vnPostError( VN_ERROR_EXECUTION_FAILURE );
	}
}

CVBitStream::CVBitStream( IN VOID * pbyBytes, UINT32 uiSize )
{
	if ( VN_FAILED( Assign( pbyBytes, uiSize ) ) )
	{
		vnPostError( VN_ERROR_EXECUTION_FAILURE );
	}
}

CVBitStream::~CVBitStream()
{
    if ( VN_FAILED( Clear() ) )
    {
        vnPostError( VN_ERROR_EXECUTION_FAILURE );
    }
}

UINT8 * CVBitStream::QueryData() CONST
{
    return m_DataStore;
}

UINT32 CVBitStream::QueryCapacity() CONST
{
	return m_DataCapacity << 3;
}

UINT32 CVBitStream::QueryOccupancy() CONST
{
	return m_uiWriteIndex - m_uiReadIndex;
}

UINT32 CVBitStream::ResizeCapacity( UINT32 uiNewSizeInBits )
{
	if ( VN_PARAM_CHECK )
	{
		if ( 0 == uiNewSizeInBits )
		{
			VN_ERR("Invalid bitstream capacity requested (%i).", uiNewSizeInBits );

			vnPostError( VN_ERROR_INVALIDARG );

			return 0;
		}
	}

    if ( VN_FAILED( Clear() ) )
    {
        vnPostError( VN_ERROR_EXECUTION_FAILURE );

        return 0;
    }

	UINT32 uiByteSize = vnAlign( uiNewSizeInBits, 8 ) >> 3;

    m_DataStore       = new UINT8[ uiByteSize ];

    if ( !m_DataStore )
    {
        VN_ERR("Failed to allocate %i bytes for a bitstream.", uiByteSize );

		vnPostError( VN_ERROR_EXECUTION_FAILURE );

		return 0;
    }

    m_DataCapacity = uiByteSize;

	return uiNewSizeInBits;
}

VN_STATUS CVBitStream::Assign( CONST CVBitStream & rvalue )
{
    //
	// We copy the data into our own buffer and adjust
	// our read/write pointers appropriately.
	//

    if ( VN_FAILED( Assign( rvalue.m_DataStore, rvalue.m_DataCapacity ) ) )
    {
        return vnPostError( VN_ERROR_EXECUTION_FAILURE );
    }

	m_uiReadIndex  = rvalue.m_uiReadIndex;
	m_uiWriteIndex = rvalue.m_uiWriteIndex;

	return VN_SUCCESS;
}

VN_STATUS CVBitStream::Assign( IN VOID * pbyBytes, UINT32 uiSize )
{
    if ( VN_PARAM_CHECK )
    {
        if ( 0 == uiSize || !pbyBytes )
        {
            return vnPostError( VN_ERROR_INVALIDARG );
        }
    }

    if ( VN_FAILED( Clear() ) )
    {
        return vnPostError( VN_ERROR_EXECUTION_FAILURE );
    }

	//
	// We copy the data into our own buffer and adjust
	// our read/write pointers appropriately.
	//

    m_DataStore = new UINT8[ uiSize ];

    if ( !m_DataStore )
    {
        return vnPostError( VN_ERROR_OUTOFMEMORY );
    }

    vnCopyMemory( m_DataStore, pbyBytes, uiSize );

	m_uiReadIndex  = 0;
	m_uiWriteIndex = uiSize << 3;
    m_DataCapacity = uiSize;

	return VN_SUCCESS;
}

VN_STATUS CVBitStream::Seek( UINT32 uiBitOffset )
{
	if ( uiBitOffset >= m_uiWriteIndex )
	{
		uiBitOffset = m_uiWriteIndex;
	}

	m_uiReadIndex = uiBitOffset;

	return VN_SUCCESS;
}

VN_STATUS CVBitStream::Clear()
{
	Empty();

    delete [] m_DataStore;

    m_DataStore    = 0;
    m_DataCapacity = 0;

	return VN_SUCCESS;
}

VN_STATUS CVBitStream::Empty()
{
	m_uiWriteIndex = 0;
	m_uiReadIndex  = 0;

	return VN_SUCCESS;
}

VN_STATUS CVBitStream::WriteBit( UINT8 uiValue )
{
	if ( m_uiWriteIndex + 1 > QueryCapacity() )
	{
		VN_WRN("Bitstream write capacity reached.");

		return VN_ERROR_CAPACITY_LIMIT;
	}

	//
	// Determine the current byte to write to
	//

	UINT32 uiTargetByte = m_uiWriteIndex >> 3;
	UINT8  uiTargetBit  = m_uiWriteIndex % 8;

	//
	// Pull the correct byte from our data store, update it, and then store it.
	// Note that we do not guarantee that unused buffer memory was zero filled,
	// thus we safely clear the write bit.
	//

	UINT8 * uiData = &( m_DataStore[ uiTargetByte ] );

	(*uiData) = ( (*uiData) & ~( 0x1 << uiTargetBit ) ) | ( uiValue & 0x1 ) << uiTargetBit;

	m_uiWriteIndex++;

	return VN_SUCCESS;
}

VN_STATUS CVBitStream::WriteByte( UINT8 uiValue )
{
    if ( m_uiWriteIndex + 8 > QueryCapacity() )
	{
		VN_WRN("Bitstream write capacity reached.");

		return VN_ERROR_CAPACITY_LIMIT;
	}

    //
	// Determine the current byte to write to
	//

	UINT32 uiTargetByte = m_uiWriteIndex >> 3;
	UINT8  uiTargetBit  = m_uiWriteIndex % 8;

    if ( 0 == uiTargetBit )
    {
        //
        // This is a byte aligned write, so we perform it at byte level.
        //

        UINT8 * uiData = &( m_DataStore[ uiTargetByte ] );

	    (*uiData) = uiValue;

        m_uiWriteIndex += 8;
    }
    else
    {
        //
        // Slower byte unaligned write.
        //

        for ( UINT8 i = 0; i < 8; i++ )
        {
            WriteBit( ( uiValue >> i ) & 0x1 );
        }
    }

    return VN_SUCCESS;
}

VN_STATUS CVBitStream::ReadBit( VOID * pData )
{
    UINT8 * pbyData = reinterpret_cast<UINT8 *>( pData );

	if ( VN_PARAM_CHECK )
	{
		if ( !pData )
		{
			return vnPostError( VN_ERROR_INVALIDARG );
		}
	}

	if ( m_uiReadIndex >= m_uiWriteIndex )
	{
		VN_WRN("Requested read operation on an empty bitstream.");

		return vnPostError( VN_ERROR_INVALID_RESOURCE );
	}

	//
	// Determine the current byte to read from
	//

	UINT32 uiTargetByte = m_uiReadIndex >> 3;
	UINT8  uiTargetBit  = m_uiReadIndex % 8;

	//
	// Pull the correct byte from our data store. Note that we
	// preserve the high bits of pbyData.
	//

	(*pbyData) &= 0xFE;
	(*pbyData) |= ( ( m_DataStore[ uiTargetByte ] ) >> uiTargetBit ) & 0x1;

	m_uiReadIndex++;

	return VN_SUCCESS;
}

VN_STATUS CVBitStream::ReadByte( VOID * pData )
{
    UINT8 * pbyData = reinterpret_cast<UINT8 *>( pData );

	if ( VN_PARAM_CHECK )
	{
		if ( !pbyData )
		{
			return vnPostError( VN_ERROR_INVALIDARG );
		}
	}

	if ( m_uiReadIndex + 8 > m_uiWriteIndex )
	{
		VN_WRN("Insufficient data to complete read request.");

		return VN_ERROR_INVALID_RESOURCE;
	}

	//
	// Determine the current byte to read from
	//

	UINT32 uiTargetByte = m_uiReadIndex >> 3;
	UINT8  uiTargetBit  = m_uiReadIndex % 8;

    if ( 0 == ( uiTargetBit % 8 ) )
    {
        (*pbyData) = m_DataStore[ uiTargetByte ];

        m_uiReadIndex += 8;
    }
    else
    {
        //
        // Slower byte unaligned read.
        //

        UINT8 uiTemp = 0;

        for ( UINT8 i = 0; i < 8; i++ )
        {
            uiTemp <<= 0x1;

            ReadBit( &uiTemp );
        }

        (*pbyData) = uiTemp;
    }

	return VN_SUCCESS;
}

UINT32 CVBitStream::AlignedCopy( UINT8 * pbyDest, UINT32 uiDestBitOffset, UINT8 * pbySource, UINT32 uiSourceBitOffset, UINT32 uiBitCountToCopy )
{
	if ( VN_PARAM_CHECK )
	{
		if ( !pbyDest || 0 != ( uiDestBitOffset % 8 ) || !pbySource || 0 != ( uiSourceBitOffset % 8 ) || 0 == ( uiBitCountToCopy >> 3 ) )
		{
			vnPostError( VN_ERROR_INVALIDARG );

			return 0;
		}
	}

	UINT32 uiDestByteOffset		= uiDestBitOffset >> 3;
	UINT32 uiSourceByteOffset	= uiSourceBitOffset >> 3;
	UINT32 uiBytesCopied		= ( uiBitCountToCopy >> 3 );

	vnCopyMemory( pbyDest + uiDestByteOffset, pbySource + uiSourceByteOffset, uiBytesCopied );

	return ( uiBytesCopied << 3 );
}

UINT32 CVBitStream::UnalignedCopy( UINT8 * pbyDest, UINT32 uiDestOffset, UINT8 * pbySource, UINT32 uiSourceOffset, UINT32 uiBitCountToCopy )
{
	if ( VN_PARAM_CHECK )
	{
		if ( !pbyDest || 0 == uiBitCountToCopy || !pbySource )
		{
			vnPostError( VN_ERROR_INVALIDARG );

			return 0;
		}
	}

	UINT32 uiSourceCopyLimit = uiSourceOffset + uiBitCountToCopy;

	//
	// Perform an unaligned copy of our data.
	//

	while ( uiSourceOffset < uiSourceCopyLimit )
	{
		UINT32 uiTargetByte = uiDestOffset >> 3;
		UINT8  uiTargetBit  = uiDestOffset % 8;
		UINT32 uiSourceByte = uiSourceOffset >> 3;
		UINT8  uiSourceBit  = uiSourceOffset % 8;

		UINT32 uiBitsLeft  = uiSourceCopyLimit - uiSourceOffset;

		//
		// We traverse our buffer and perform copies in as large of increments as possible.
		//

		UINT8 uiWriteCapacity   = VN_MIN2( 8 - uiTargetBit, 8 - uiSourceBit );
		UINT8 uiWriteCount	    = VN_MIN2( uiWriteCapacity, uiBitsLeft );
		UINT8 uiWriteFillMask	= ( 0x1 << uiWriteCount ) - 1;

		UINT8 * pTargetData		= &( pbyDest[ uiTargetByte ]);
		UINT8 * pSourceData		= &( pbySource[ uiSourceByte ] );

		(*pTargetData)			= ( (*pTargetData) & ~( uiWriteFillMask << uiTargetBit ) );
		(*pTargetData)		   |= ( ( (*pSourceData) & ( uiWriteFillMask << uiSourceBit ) ) >> uiSourceBit ) << uiTargetBit;

		uiSourceOffset		   += uiWriteCount;
		uiDestOffset		   += uiWriteCount;
	}

	return uiBitCountToCopy;
}

VN_STATUS CVBitStream::WriteBits( VOID * pData, UINT32 uiBitCount )
{
    UINT8 * pbyData = reinterpret_cast<UINT8 *>( pData );

	if ( VN_PARAM_CHECK )
	{
		if ( !pbyData || 0 == uiBitCount )
		{
			return vnPostError( VN_ERROR_INVALIDARG );
		}
	}

	if ( m_uiWriteIndex + uiBitCount > QueryCapacity() )
	{
		VN_WRN("Bitstream write capacity reached.");

		return VN_ERROR_CAPACITY_LIMIT;
	}

	UINT32 uiBitsCopied = 0;

	if ( 0 == ( m_uiWriteIndex % 8 ) && ( uiBitCount >= 8 ) )
	{
		//
		// We can perform a (partial) fast copy because our source and destination
		// are byte aligned. Note that this may leave trailing bits that still need
		// to be copied below.
		//

		uiBitsCopied = AlignedCopy( m_DataStore, m_uiWriteIndex, pbyData, 0, uiBitCount );

		if ( !uiBitsCopied )
		{
			return vnPostError( VN_ERROR_EXECUTION_FAILURE );
		}
	}

	//
	// Perform unaligned copies of our data
	//

	if ( uiBitsCopied < uiBitCount )
	{
		uiBitsCopied += UnalignedCopy( m_DataStore, m_uiWriteIndex + uiBitsCopied, pbyData, uiBitsCopied, uiBitCount - uiBitsCopied );
	}

	m_uiWriteIndex += uiBitsCopied;

	return VN_SUCCESS;
}

VN_STATUS CVBitStream::WriteBytes( VOID * pData, UINT32 uiByteCount )
{
    UINT8 * pbyData = reinterpret_cast<UINT8 *>( pData );

	if ( VN_PARAM_CHECK )
	{
		if ( !pbyData || 0 == uiByteCount )
		{
			return vnPostError( VN_ERROR_INVALIDARG );
		}
	}

	if ( m_uiWriteIndex + ( uiByteCount << 3 ) > QueryCapacity() )
	{
		VN_WRN("Bitstream write capacity reached.");

		return VN_ERROR_CAPACITY_LIMIT;
	}

	if ( 0 == ( m_uiWriteIndex % 8 ) )
	{
		//
		// We can perform a fast copy because our source is byte aligned.
		//

        UINT8 * pDest = m_DataStore + ( m_uiWriteIndex >> 3 );

		vnCopyMemory( pDest, pbyData, uiByteCount );

        m_uiWriteIndex += uiByteCount << 3;
	}
    else
    {
        WriteBits( pbyData, uiByteCount << 3 );
    }

	return VN_SUCCESS;
}

VN_STATUS CVBitStream::ReadBits( VOID * pData, INOUT UINT32 * puiBitCount )
{
    UINT8 * pbyData = reinterpret_cast<UINT8 *>( pData );

	if ( VN_PARAM_CHECK )
	{
		if ( !pbyData || !puiBitCount || 0 == (*puiBitCount) )
		{
			return vnPostError( VN_ERROR_INVALIDARG );
		}
	}

	//
	// We attempt to read *puiBitCount bits and replace it with the number
	// actually read.
	//

	if ( m_uiReadIndex + *puiBitCount > m_uiWriteIndex )
	{
		(*puiBitCount) = m_uiWriteIndex - m_uiReadIndex;
	}

	UINT32 uiBitsCopied = 0;

	if ( 0 == ( m_uiReadIndex % 8 ) && ( (*puiBitCount) >= 8 ) )
	{
		//
		// We can perform a (partial) fast copy because our source and destination
		// are byte aligned. Note that this may leave trailing bits that still need
		// to be copied below.
		//

		uiBitsCopied = AlignedCopy( pbyData, 0, m_DataStore, m_uiReadIndex, (*puiBitCount) );

		if ( !uiBitsCopied )
		{
			return vnPostError( VN_ERROR_EXECUTION_FAILURE );
		}
	}

	//
	// Perform unaligned copies of our data
	//

	if ( uiBitsCopied < (*puiBitCount) )
	{
		uiBitsCopied += UnalignedCopy( pbyData, uiBitsCopied, m_DataStore, m_uiReadIndex + uiBitsCopied, (*puiBitCount) - uiBitsCopied );
	}

	m_uiReadIndex += uiBitsCopied;

	return VN_SUCCESS;
}

VN_STATUS CVBitStream::ReadBytes( VOID * pData, INOUT UINT32 * puiByteCount )
{
    UINT8 * pbyData = reinterpret_cast<UINT8 *>( pData );

	if ( VN_PARAM_CHECK )
	{
		if ( !pbyData || !puiByteCount || 0 == (*puiByteCount) )
		{
			return vnPostError( VN_ERROR_INVALIDARG );
		}
	}

	//
	// We attempt to read *puiBitCount bits and replace it with the number
	// actually read.
	//

	if ( m_uiReadIndex + ((*puiByteCount) << 3 ) > m_uiWriteIndex )
	{
		(*puiByteCount) = ( m_uiWriteIndex - m_uiReadIndex ) >> 3;

        if ( !(*puiByteCount) )
        {
            return VN_ERROR_CAPACITY_LIMIT;
        }
	}

	if ( 0 == ( m_uiReadIndex % 8 ) )
	{
		//
		// We can perform a fast copy because our source is byte aligned.
		//

        UINT8 * pSrc = m_DataStore + ( m_uiReadIndex >> 3 );

		vnCopyMemory( pbyData, pSrc, *puiByteCount );

        m_uiReadIndex += (*puiByteCount) << 3;
	}
    else
    {
        //
        // We don't bother checking uiBitCount after the read because we've
        // already checked the capacity limits.
        //

        UINT32 uiBitCount = (*puiByteCount) << 3;

        ReadBits( pbyData, &uiBitCount );
    }

	return VN_SUCCESS;
}

BOOL CVBitStream::IsEmpty() CONST
{
	return ( m_uiWriteIndex == m_uiReadIndex );
}

BOOL CVBitStream::IsFull() CONST
{
	return ( m_uiWriteIndex == QueryCapacity() );
}

CVBitStream & CVBitStream::operator = ( CONST CVBitStream & rvalue )
{
	if ( VN_FAILED( Assign( rvalue ) ) )
	{
		vnPostError( VN_ERROR_EXECUTION_FAILURE );
	}

	return (*this);
}

BOOL CVBitStream::operator == ( CONST CVBitStream & rvalue ) CONST
{
    //
    // Two bitstreams are only identical if their entire (deep) state is
    // the same.
    //

	if ( m_uiWriteIndex != rvalue.m_uiWriteIndex ) return FALSE;
	if ( m_uiReadIndex  != rvalue.m_uiReadIndex )  return FALSE;
	if ( m_DataCapacity != rvalue.m_DataCapacity ) return FALSE;

    for ( UINT32 i = 0; i < m_DataCapacity; i++ )
    {
        if ( m_DataStore[ i ] != rvalue.m_DataStore[ i ] )
        {
            return FALSE;
        }
    }

	return TRUE;
}

BOOL CVBitStream::operator != ( CONST CVBitStream & rvalue ) CONST
{
	return !( (*this) == rvalue );
}
unsigned reverse(unsigned x){unsigned v=0;for(int i=0;i<8;i++)v=(v<<1)|((x>>i)&1);return v;}
int main(){
 for(unsigned v=0;v<256;v++){
  CVBitStream s;s.ResizeCapacity(16);assert(s.WriteByte(v)==0);UINT8 out=0;
  assert(s.ReadByte(&out)==0 && out==v);
 }
 std::cout<<"PASS aligned byte roundtrip for all256 values\n";
 for(unsigned v=0;v<256;v++){
  CVBitStream s;s.ResizeCapacity(16);s.WriteBit(1);s.WriteByte(v);s.Seek(1);UINT8 out=0;
  assert(s.ReadByte(&out)==0 && out==reverse(v));
 }
 std::cout<<"PASS unaligned ReadByte reverses all256 values relative to WriteByte\n";
 for(unsigned v=0;v<256;v++){
  CVBitStream s;s.ResizeCapacity(16);s.WriteBit(1);s.WriteByte(v);s.Seek(1);UINT8 out=0;UINT32 n=1;
  assert(s.ReadBytes(&out,&n)==0 && n==1 && out==v);
 }
 std::cout<<"PASS unaligned bulk ReadBytes preserves all256 values\n";
 CVBitStream s;s.ResizeCapacity(8);s.WriteByte(19);assert(s.WriteBit(1)==VN_ERROR_CAPACITY_LIMIT);
 UINT8 out=0;assert(s.ReadByte(&out)==0 && out==19 && s.IsEmpty());
 std::cout<<"PASS capacity refusal preserves stored byte and occupancy\n";
}
