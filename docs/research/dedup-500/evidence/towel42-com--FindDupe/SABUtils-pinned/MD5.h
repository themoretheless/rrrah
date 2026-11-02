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

#ifndef __COMMON_MD5_H
#define __COMMON_MD5_H

#include "SABUtilsExport.h"

class QByteArray;
class QFileInfo;
class QString;
class QIcon;
class QPixmap;
class QImage;
#include <QRunnable>
#include <string>
#include <QObject>
#include <QFileInfo>

namespace NSABUtils
{
    SABUTILS_EXPORT QByteArray getMd5( const QByteArray &data );
    SABUTILS_EXPORT QByteArray getMd5( const QFileInfo &fi );
    SABUTILS_EXPORT QByteArray getMd5( const QStringList &data );
    SABUTILS_EXPORT QByteArray getMd5( const QString &data, bool isFileName = false );
    SABUTILS_EXPORT std::string getMd5( const std::string &data, bool isFileName = false );

    SABUTILS_EXPORT QByteArray getMd5( const QIcon &icon );
    SABUTILS_EXPORT QByteArray getMd5( const QPixmap &pixmap ); // only includes image data
    SABUTILS_EXPORT QByteArray getMd5( const QImage &img );
    SABUTILS_EXPORT QByteArray getImageData( const QImage &img );

    SABUTILS_EXPORT QByteArray formatMd5( const QByteArray &digest, bool isHex );

    class SABUTILS_EXPORT CComputeMD5 : public QObject, public QRunnable
    {
        Q_OBJECT;

    public:
        CComputeMD5( const QString &fileName );;

        void run() override;

    public:
        unsigned long long getThreadID() const;

        QByteArray md5() const { return fMD5; }

        void stop() { slotStop(); }
    Q_SIGNALS:
        void sigStarted( unsigned long long threadID, const QDateTime &dt, const QString &filename );
        void sigFinishedReading( unsigned long long threadID, const QDateTime &dt, const QString &filename );
        void sigReadPositionStatus( unsigned long long threadID, const QDateTime &dt, const QString &filename, qint64 pos );
        void sigFinishedComputing( unsigned long long threadID, const QDateTime &dt, const QString &filename );
        void sigFinished( unsigned long long threadID, const QDateTime &dt, const QString &filename, const QString &md5 );
    public Q_SLOTS:
        void slotStop();

    private:
        void processNonImage();

    private:
        void processEvents();

        void processImage( const QImage &img );
        void emitFinished();

        QFileInfo fFileInfo;
        QByteArray fMD5;
        bool fStopped{ false };
    };

}

#endif
