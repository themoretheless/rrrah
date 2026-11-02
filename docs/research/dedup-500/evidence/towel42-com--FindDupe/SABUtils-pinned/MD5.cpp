// The MIT License( MIT )
//
// Copyright( c ) 2020-2021 Scott Aron Bloom
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files( the "Software" ), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sub-license, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions :
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

#include "MD5.h"

#include <QString>
#include <QCryptographicHash>
#include <QFileInfo>
#include <QThread>
#include <QDateTime>
#include <QDebug>
#include <QAbstractEventDispatcher>
#include <QIcon>
#include <QBuffer>

#include <QImageWriter>
#include <QDataStream>

namespace NSABUtils
{
    QByteArray formatMd5( const QByteArray &digest, bool isHex )
    {
        QByteArray md5Str = digest;
        if ( !isHex )
        {
            md5Str = md5Str.toHex();
        }
        md5Str = md5Str.toUpper();
        if ( md5Str.length() < 32 )
            md5Str = QString( "%1" ).arg( QString::fromLatin1( md5Str ), 32, QLatin1Char( '0' ) ).toLatin1();
        if ( md5Str.length() == 32 )
        {
            md5Str = md5Str.mid( 0, 8 ) + "-" + md5Str.mid( 8, 4 ) + "-" + md5Str.mid( 12, 4 ) + "-" + md5Str.mid( 16, 4 ) + "-" + md5Str.mid( 20 );
        }
        else
        {
            for ( int ii = md5Str.length() - 4; ii > 0; ii -= 4 )
            {
                md5Str.insert( ii, '-' );
            }
        }
        return md5Str;
    }

    QByteArray getMd5( const QByteArray &data )
    {
        auto digest = QCryptographicHash::hash( data, QCryptographicHash::Md5 );
        return formatMd5( digest, false );
    }

    QByteArray getMd5( const QStringList &data )
    {
        QCryptographicHash hash( QCryptographicHash::Md5 );
        for ( auto &&ii : data )
        {
            hash.addData( ii.toLatin1() );
        }
        return hash.result();
    }

    QByteArray getMd5( const QString &data, bool isFileName )
    {
        if ( isFileName )
            return getMd5( QFileInfo( data ) );

        QByteArray inData = data.toLatin1();
        return getMd5( inData, false );
    }

    std::string getMd5( const std::string &data, bool isFileName )
    {
        return getMd5( QString::fromStdString( data ), isFileName ).toStdString();
    }

    QByteArray getMd5( const QFileInfo &fi )
    {
        QFile file( fi.absoluteFilePath() );
        if ( !file.open( QIODevice::ReadOnly ) )
            return {};

        QCryptographicHash hash( QCryptographicHash::Md5 );
        if ( hash.addData( &file ) )
            return formatMd5( hash.result(), false );

        return {};
    }

    SABUTILS_EXPORT QByteArray getMd5( const QPixmap &pixMap )
    {
        return getMd5( pixMap.toImage() );
    }

    SABUTILS_EXPORT QByteArray getMd5( const QIcon &icon )
    {
        auto sizes = icon.availableSizes();
        QCryptographicHash hash( QCryptographicHash::Md5 );
        for ( auto &&ii : sizes )
        {
            auto data = getImageData( icon.pixmap( ii ).toImage() );
            hash.addData( data );
        }
        return formatMd5( hash.result(), false );
    }

    SABUTILS_EXPORT QByteArray getMd5( const QImage &image )
    {
        QCryptographicHash hash( QCryptographicHash::Md5 );
        hash.addData( getImageData( image ) );
        return formatMd5( hash.result(), false );
    }

    SABUTILS_EXPORT QByteArray getImageData( const QImage &img )
    {
        if ( img.isNull() )
            return {};

        QByteArray retVal;
        QDataStream ds( &retVal, QDataStream::WriteOnly );

        auto width = img.width();
        auto height = img.height();
        auto depth = img.depth();
        ds << width << height << depth;

        auto bpl = img.bytesPerLine();

        if ( ( img.format() == QImage::Format::Format_RGB32 ) || ( img.format() >= QImage::Format::Format_ARGB32 ) )
        {
            if ( ( img.format() >= QImage::Format::Format_ARGB32 ) && ( ( width * depth / 8 ) == bpl ) )
            {
                ds.writeBytes( reinterpret_cast< const char * >( img.bits() ), bpl );
            }
            else
            {
                for ( int line = 0; line < height; ++line )
                {
                    ds.writeBytes( reinterpret_cast< const char * >( img.constScanLine( line ) ), bpl );
                }
            }
        }
        else
        {
            const QVector< QRgb > &colortable = img.colorTable();
            for ( int y = 0; y < height; ++y )
            {
                for ( int x = 0; x < width; ++x )
                {
                    auto curr = colortable[ img.pixelIndex( x, y ) ];
                    retVal.push_back( QByteArray( reinterpret_cast< const char * >( &curr ), sizeof( QRgb ) ) );
                }
            }
        }
        return retVal;
    }
    CComputeMD5::CComputeMD5( const QString &fileName ) :
        fFileInfo( fileName )
    {
    }

    void CComputeMD5::run()
    {
        emit sigStarted( getThreadID(), QDateTime::currentDateTime(), fFileInfo.absoluteFilePath() );

        QImage img( fFileInfo.absoluteFilePath() );
        if ( img.isNull() )
            processNonImage();
        else
            processImage( img );

        emitFinished();
    
    }

    unsigned long long CComputeMD5::getThreadID() const
    {
        return reinterpret_cast< unsigned long long >( QThread::currentThreadId() );
    }

    void CComputeMD5::slotStop()
    {
        fStopped = true;
        // qDebug() << "ComputeMD5 stopped";
    }

    void CComputeMD5::emitFinished()
    {
        emit sigFinished( getThreadID(), QDateTime::currentDateTime(), fFileInfo.absoluteFilePath(), fMD5 );
    }

    void CComputeMD5::processImage( const QImage &img )
    {
        fMD5 = getMd5( img );
        emit sigFinishedReading( getThreadID(), QDateTime::currentDateTime(), fFileInfo.absoluteFilePath() );
        emit sigFinishedComputing( getThreadID(), QDateTime::currentDateTime(), fFileInfo.absoluteFilePath() );
    }

    void CComputeMD5::processNonImage()
    {
        QFile file( fFileInfo.absoluteFilePath() );
        if ( !file.open( QIODevice::ReadOnly ) || !file.isReadable() )
            return;

        QCryptographicHash hash( QCryptographicHash::Md5 );

        QByteArray buffer;

        qint64 pos = 0;
        while ( !fStopped && ( buffer = file.read( 4096 ) ).length() > 0 )
        {
            pos += buffer.length();
            hash.addData( buffer );
            emit sigReadPositionStatus( getThreadID(), QDateTime::currentDateTime(), fFileInfo.absoluteFilePath(), pos );
            processEvents();
        }

        if ( file.atEnd() )
        {
            emit sigFinishedReading( getThreadID(), QDateTime::currentDateTime(), fFileInfo.absoluteFilePath() );
            if ( fStopped )
            {
                emitFinished();
                return;
            }

            auto tmp = hash.result();
            emit sigFinishedComputing( getThreadID(), QDateTime::currentDateTime(), fFileInfo.absoluteFilePath() );
            if ( fStopped )
            {
                emitFinished();
                return;
            }

            fMD5 = formatMd5( tmp, false );
        }
    }

    void CComputeMD5::processEvents()
    {
        if ( QThread::currentThread() && QThread::currentThread()->eventDispatcher() )
        {
            QThread::currentThread()->eventDispatcher()->processEvents( QEventLoop::AllEvents );
        }
    }

}
