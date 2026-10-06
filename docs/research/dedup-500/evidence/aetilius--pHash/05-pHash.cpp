/*

    pHash, the open source perceptual hash library
    Copyright (C) 2009 Aetilius, Inc.
    All rights reserved.

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <http://www.gnu.org/licenses/>.

    Evan Klinger - eklinger@phash.org
    D Grant Starkweather - dstarkweather@phash.org

*/

#include "pHash.h"
#ifdef HAVE_VIDEO_HASH
#include "cimgffmpeg.h"
#endif
#include "ph_heif.h"

#include <cmath>
#include <map>
#include <vector>

namespace {

// Single entry point for "load any supported image file into a
// CImg<uint8_t>". HEIF/HEIC/AVIF files are routed to libheif (when
// HAVE_HEIF is defined); everything else goes through CImg's own loader
// (libpng / libjpeg / libtiff). Returns 0 on success, non-zero on
// failure. CImg::load can still throw on truly malformed input -- callers
// should wrap the call in try/catch.
int load_image(const char *file, CImg<uint8_t> &img) {
    if (file == NULL) return -1;
    if (ph_is_heif_path(file)) {
#ifdef HAVE_HEIF
        return ph_load_heif(file, img);
#else
        return -1;  // HEIF path but library wasn't built with libheif
#endif
    }
    img.load(file);
    return 0;
}

}  // namespace

const char phash_project[] = "%s. Copyright 2008-2010 Aetilius, Inc.";
char phash_version[255] = {0};
const char *ph_about() {
    if (phash_version[0] != 0) return phash_version;

    snprintf(phash_version, sizeof(phash_version), phash_project,
             PACKAGE_STRING);
    return phash_version;
}
#ifdef HAVE_IMAGE_HASH
// Sample the image at (x, y) using bilinear interpolation. (x, y) must lie
// inside [0, width-1] x [0, height-1]; caller checks bounds.
static inline double bilinear_sample(const CImg<uint8_t> &img, double x,
                                     double y) {
    int x0 = (int)std::floor(x);
    int y0 = (int)std::floor(y);
    int x1 = (x0 + 1 < img.width()) ? x0 + 1 : x0;
    int y1 = (y0 + 1 < img.height()) ? y0 + 1 : y0;
    double fx = x - x0;
    double fy = y - y0;
    double v00 = img(x0, y0);
    double v10 = img(x1, y0);
    double v01 = img(x0, y1);
    double v11 = img(x1, y1);
    return (1 - fx) * (1 - fy) * v00 + fx * (1 - fy) * v10 +
           (1 - fx) * fy * v01 + fx * fy * v11;
}

// Radon-transform-style line integrals: for each of N angles uniformly
// spaced in [0, pi), walk along a line through the image center at that
// angle, sampling with bilinear interpolation. Stores up to D samples per
// angle in projs.R (column = angle k, row = position along the line).
// nb_pix_perline[k] is the count of in-bounds samples for angle k.
//
// The previous implementation walked integer pixels with separate
// x-stepping / y-stepping branches stitched together at 45 / 135 degrees,
// which left projections asymmetric, missed sub-pixel detail, and contained
// a reflected (rather than rotated) line for one of the quadrants. This
// version uses a single uniform parameterisation per angle.
int ph_radon_projections(const CImg<uint8_t> &img, int N, Projections &projs) {
    projs.R = NULL;
    projs.nb_pix_perline = NULL;
    projs.size = 0;
    if (N <= 0) return -1;

    int width = img.width();
    int height = img.height();
    if (width <= 0 || height <= 0) return -1;
    int D = (width > height) ? width : height;
    double cx = (width - 1) / 2.0;
    double cy = (height - 1) / 2.0;

    try {
        projs.R = new CImg<uint8_t>(N, D, 1, 1, 0);
    } catch (...) {
        return -1;
    }
    projs.nb_pix_perline = (int *)calloc(N, sizeof(int));
    if (!projs.nb_pix_perline) {
        delete projs.R;
        projs.R = NULL;
        return -1;
    }
    projs.size = N;

    CImg<uint8_t> *ptr_radon_map = projs.R;
    int *nb_per_line = projs.nb_pix_perline;
    int half = D / 2;

    for (int k = 0; k < N; k++) {
        double theta = (double)k * cimg::PI / (double)N;
        double dx = std::cos(theta);
        double dy = std::sin(theta);

        for (int i = 0; i < D; i++) {
            double t = (double)(i - half);
            double x = cx + t * dx;
            double y = cy + t * dy;

            if (x >= 0.0 && x <= (double)(width - 1) && y >= 0.0 &&
                y <= (double)(height - 1)) {
                double v = bilinear_sample(img, x, y);
                if (v < 0.0) v = 0.0;
                if (v > 255.0) v = 255.0;
                *ptr_radon_map->data(k, i) = (uint8_t)(v + 0.5);
                nb_per_line[k] += 1;
            }
        }
    }

    return 0;
}
int ph_feature_vector(const Projections &projs, Features &fv) {
    fv.features = NULL;
    fv.size = 0;
    if (projs.R == NULL || projs.nb_pix_perline == NULL || projs.size <= 0) {
        return -1;
    }

    CImg<uint8_t> *ptr_map = projs.R;
    CImg<uint8_t> projection_map = *ptr_map;
    int *nb_perline = projs.nb_pix_perline;
    int N = projs.size;
    int D = projection_map.height();

    fv.features = (double *)malloc(N * sizeof(double));
    if (!fv.features) return -1;
    fv.size = N;

    double *feat_v = fv.features;
    double sum = 0.0;
    double sum_sqd = 0.0;
    for (int k = 0; k < N; k++) {
        double line_sum = 0.0;
        double line_sum_sqd = 0.0;
        int nb_pixels = nb_perline[k];
        for (int i = 0; i < D; i++) {
            line_sum += projection_map(k, i);
            line_sum_sqd += projection_map(k, i) * projection_map(k, i);
        }
        if (nb_pixels > 0) {
            feat_v[k] = (line_sum_sqd / nb_pixels) -
                        (line_sum * line_sum) /
                            ((double)nb_pixels * nb_pixels);
        } else {
            feat_v[k] = 0.0;
        }
        sum += feat_v[k];
        sum_sqd += feat_v[k] * feat_v[k];
    }
    double mean = sum / N;
    double variance_term = (sum_sqd / N) - (sum * sum) / ((double)N * N);
    double var = (variance_term > 0.0) ? sqrt(variance_term) : 0.0;
    if (var == 0.0) {
        for (int i = 0; i < N; i++) feat_v[i] = 0.0;
    } else {
        for (int i = 0; i < N; i++) {
            feat_v[i] = (feat_v[i] - mean) / var;
        }
    }

    return 0;
}
int ph_dct(const Features &fv, Digest &digest) {
    digest.coeffs = NULL;
    digest.size = 0;
    if (fv.features == NULL || fv.size <= 0) return -1;

    int N = fv.size;
    const int nb_coeffs = 40;

    digest.coeffs = (uint8_t *)malloc(nb_coeffs * sizeof(uint8_t));
    if (!digest.coeffs) return -1;

    digest.size = nb_coeffs;

    double *R = fv.features;

    uint8_t *D = digest.coeffs;

    double D_temp[nb_coeffs];
    double max = 0.0;
    double min = 0.0;
    double sqrt_n = sqrt((double)N);
    for (int k = 0; k < nb_coeffs; k++) {
        double sum = 0.0;
        for (int n = 0; n < N; n++) {
            double temp = R[n] * cos((cimg::PI * (2 * n + 1) * k) / (2 * N));
            sum += temp;
        }
        if (k == 0)
            D_temp[k] = sum / sqrt_n;
        else
            D_temp[k] = sum * SQRT_TWO / sqrt_n;
        if (D_temp[k] > max) max = D_temp[k];
        if (D_temp[k] < min) min = D_temp[k];
    }

    double range = max - min;
    if (range > 0.0) {
        for (int i = 0; i < nb_coeffs; i++) {
            D[i] = (uint8_t)(UCHAR_MAX * (D_temp[i] - min) / range);
        }
    } else {
        for (int i = 0; i < nb_coeffs; i++) D[i] = 0;
    }

    return 0;
}

int ph_crosscorr(const Digest &x, const Digest &y, double &pcc,
                 double threshold) {
    pcc = 0;
    if (x.coeffs == NULL || y.coeffs == NULL || x.size <= 0 ||
        y.size != x.size) {
        return -1;
    }

    int N = y.size;
    uint8_t *x_coeffs = x.coeffs;
    uint8_t *y_coeffs = y.coeffs;

    double sumx = 0.0;
    double sumy = 0.0;
    for (int i = 0; i < N; i++) {
        sumx += x_coeffs[i];
        sumy += y_coeffs[i];
    }
    double meanx = sumx / N;
    double meany = sumy / N;

    // Denominators don't depend on lag d -- compute once. Both denoms are
    // identical (same set of values, rearranged by a cyclic shift).
    double den = 0.0;
    for (int i = 0; i < N; i++) {
        double dx = x_coeffs[i] - meanx;
        den += dx * dx;
    }
    if (den <= 0.0) {
        pcc = 0;
        return 0;
    }

    double max = -1.0;
    for (int d = 0; d < N; d++) {
        double num = 0.0;
        for (int i = 0; i < N; i++) {
            num += (x_coeffs[i] - meanx) *
                   (y_coeffs[(N + i - d) % N] - meany);
        }
        double r = num / den;
        if (r > max) max = r;
    }
    pcc = max;
    return (max > threshold) ? 1 : 0;
}

#ifdef max
#undef max
#endif

int _ph_image_digest(const CImg<uint8_t> &img, double sigma, double gamma,
                     Digest &digest, int N) {
    Projections projs = {};
    Features features = {};
    int result = -1;

    CImg<uint8_t> graysc;
    if (img.spectrum() == 3) {
        graysc = img.get_RGBtoYCbCr().channel(0);
    } else if (img.spectrum() > 3) {
        // RGBA (or wider): drop alpha and any extra channels before
        // RGBtoYCbCr, which requires exactly 3 channels and would
        // otherwise throw CImgInstanceException (issue #39).
        graysc = img.get_channels(0, 2).RGBtoYCbCr().channel(0);
    } else if (img.spectrum() == 1 || img.spectrum() == 2) {
        // 1-channel grayscale or 2-channel gray+alpha: just take the
        // luminance channel.
        graysc = img.get_channel(0);
    } else {
        return -1;
    }

    graysc.blur((float)sigma);

    // Gamma correction. The original expression
    //     (graysc / graysc.max()).pow(gamma);
    // discarded its result -- CImg::operator/ returns a new image, and
    // pow() then modified that temporary which was thrown away at the
    // semicolon, so graysc was never actually gamma-corrected (issue
    // #22). Do the work in float space so the curve isn't quantised at
    // each step, then convert back to the 8-bit pixel type radon expects.
    if (gamma != 1.0) {
        CImg<float> tmp(graysc);
        float maxv = tmp.max();
        if (maxv > 0) tmp /= maxv;
        tmp.pow(gamma);
        tmp *= 255.0f;
        graysc = tmp;
    }

    if (ph_radon_projections(graysc, N, projs) < 0) goto cleanup;
    if (ph_feature_vector(projs, features) < 0) goto cleanup;
    if (ph_dct(features, digest) < 0) goto cleanup;

    result = 0;

cleanup:
    free(projs.nb_pix_perline);
    free(features.features);
    delete projs.R;
    return result;
}

#define max(a, b) (((a) > (b)) ? (a) : (b))

int ph_image_digest(const char *file, double sigma, double gamma,
                    Digest &digest, int N) {
    if (file == NULL) return -1;
    try {
        CImg<uint8_t> src;
        if (load_image(file, src) < 0) return -1;
        return _ph_image_digest(src, sigma, gamma, digest, N);
    } catch (CImgIOException &) {
        return -1;
    } catch (CImgException &) {
        return -1;
    }
}

int _ph_compare_images(const CImg<uint8_t> &imA, const CImg<uint8_t> &imB,
                       double &pcc, double sigma, double gamma, int N,
                       double threshold) {
    Digest digestA = {};
    Digest digestB = {};
    int result = -1;

    if (_ph_image_digest(imA, sigma, gamma, digestA, N) < 0) goto cleanup;
    if (_ph_image_digest(imB, sigma, gamma, digestB, N) < 0) goto cleanup;
    if (ph_crosscorr(digestA, digestB, pcc, threshold) < 0) goto cleanup;

    result = (pcc > threshold) ? 1 : 0;

cleanup:
    free(digestA.coeffs);
    free(digestB.coeffs);
    return result;
}

int ph_compare_images(const char *file1, const char *file2, double &pcc,
                      double sigma, double gamma, int N, double threshold) {
    if (file1 == NULL || file2 == NULL) return -1;
    try {
        CImg<uint8_t> imA, imB;
        if (load_image(file1, imA) < 0) return -1;
        if (load_image(file2, imB) < 0) return -1;
        return _ph_compare_images(imA, imB, pcc, sigma, gamma, N, threshold);
    } catch (CImgIOException &) {
        return -1;
    } catch (CImgException &) {
        return -1;
    }
}

static CImg<float> ph_dct_matrix(const int N) {
    CImg<float> matrix(N, N, 1, 1, 1 / sqrt((float)N));
    const float c1 = sqrt(2.0 / N);
    for (int x = 0; x < N; x++) {
        for (int y = 1; y < N; y++) {
            matrix(x, y) = c1 * cos((cimg::PI / 2 / N) * y * (2 * x + 1));
        }
    }
    return matrix;
}

// Function-local static gives thread-safe one-time init (C++11) and avoids
// running ph_dct_matrix as a global ctor before main(), where a throw would
// abort the program.
static const CImg<float> &get_dct_matrix() {
    static const CImg<float> matrix = ph_dct_matrix(32);
    return matrix;
}
int ph_dct_imagehash(const char *file, ulong64 &hash) {
    hash = 0;
    if (!file) {
        return -1;
    }
    CImg<uint8_t> src;
    try {
        if (load_image(file, src) < 0) return -1;
    } catch (CImgIOException &) {
        return -1;
    } catch (CImgException &) {
        return -1;
    }
    CImg<float> meanfilter(7, 7, 1, 1, 1);
    CImg<float> img;
    if (src.spectrum() == 3) {
        img = src.RGBtoYCbCr().channel(0).get_convolve(meanfilter);
    } else if (src.spectrum() == 4) {
        int width = src.width();
        int height = src.height();
        img = src.crop(0, 0, 0, 0, width - 1, height - 1, 0, 2)
                  .RGBtoYCbCr()
                  .channel(0)
                  .get_convolve(meanfilter);
    } else {
        img = src.channel(0).get_convolve(meanfilter);
    }

    img.resize(32, 32);
    const CImg<float> &C = get_dct_matrix();
    CImg<float> Ctransp = C.get_transpose();

    CImg<float> dctImage = C * img * Ctransp;

    // Canonical pHash (Krawetz "Looks Like It" / Zauner 2010): take the
    // top-left 8x8 block of the DCT and bit-test each coefficient against
    // the median of the *other 63*, excluding the DC term at (0,0).
    //
    // Earlier versions of this code took (1,1)..(8,8), skipping the entire
    // first row and column of low frequencies (incompatible with every
    // other library that calls itself pHash), then took the median over
    // all 64 values. Including DC in the median was problematic too:
    // |DC| is typically 10-100x larger than any AC coefficient, so its
    // hash bit was effectively always 1, wasting one of the 64 hash bits.
    CImg<float> subsec = dctImage.crop(0, 0, 7, 7).unroll('x');

    // Median of the 63 AC coefficients (positions 1..63). Skipping DC
    // here means none of the 64 emitted bits is a foregone conclusion.
    CImg<float> ac = subsec.get_crop(1, 0, 0, 0, 63, 0, 0, 0);
    float median = ac.median();
    hash = 0;
    for (int i = 0; i < 64; i++) {
        if (subsec(i) > median) hash |= 0x01;
        if (i < 63) hash <<= 1;
    }

    return 0;
}

#endif

#if defined(HAVE_VIDEO_HASH) && defined(HAVE_IMAGE_HASH)

CImgList<uint8_t> *ph_getKeyFramesFromVideo(const char *filename) {
    long N = GetNumberVideoFrames(filename);
    if (N <= 0) {
        return NULL;
    }

    float frames_per_sec = 0.5 * fps(filename);
    if (frames_per_sec < 0) {
        return NULL;
    }

    int step = std::round(frames_per_sec);
    long nbframes = (long)(N / step);
    // If the video length is less than 1 the video is probably corrupted.
    if (nbframes <= 0) {
        return NULL;
    }

    float *dist = (float *)malloc((nbframes) * sizeof(float));
    if (!dist) {
        return NULL;
    }
    CImg<float> prev(64, 1, 1, 1, 0);

    VFInfo st_info;
    st_info.filename = filename;
    st_info.nb_retrieval = 100;
    st_info.step = step;
    st_info.pixelformat = 0;
    st_info.pFormatCtx = NULL;
    st_info.width = -1;
    st_info.height = -1;

    CImgList<uint8_t> *pframelist = new CImgList<uint8_t>();
    if (!pframelist) {
        return NULL;
    }
    int nbread = 0;
    int k = 0;
    do {
        nbread = NextFrames(&st_info, pframelist);
        if (nbread < 0) {
            delete pframelist;
            free(dist);
            return NULL;
        }
        unsigned int i = 0;
        while ((i < pframelist->size()) && (k < nbframes)) {
            CImg<uint8_t> current = pframelist->at(i++);
            CImg<float> hist = current.get_histogram(64, 0, 255);
            float d = 0.0;
            dist[k] = 0.0;
            cimg_forX(hist, X) {
                d = hist(X) - prev(X);
                d = (d >= 0) ? d : -d;
                dist[k] += d;
                prev(X) = hist(X);
            }
            k++;
        }
        pframelist->clear();
    } while ((nbread >= st_info.nb_retrieval) && (k < nbframes));
    vfinfo_close(&st_info);

    int S = 10;
    int L = 50;
    int alpha1 = 3;
    int alpha2 = 2;
    int s_begin, s_end;
    int l_begin, l_end;
    uint8_t *bnds = (uint8_t *)malloc(nbframes * sizeof(uint8_t));
    if (!bnds) {
        delete pframelist;
        free(dist);
        return NULL;
    }

    int nbboundaries = 0;
    k = 1;
    bnds[0] = 1;
    do {
        s_begin = (k - S >= 0) ? k - S : 0;
        s_end = (k + S < nbframes) ? k + S : nbframes - 1;
        l_begin = (k - L >= 0) ? k - L : 0;
        l_end = (k + L < nbframes) ? k + L : nbframes - 1;

        /* get global average */
        float ave_global, sum_global = 0.0, dev_global = 0.0;
        for (int i = l_begin; i <= l_end; i++) {
            sum_global += dist[i];
        }
        ave_global = sum_global / ((float)(l_end - l_begin + 1));

        /*get global deviation */
        for (int i = l_begin; i <= l_end; i++) {
            float dev = ave_global - dist[i];
            dev = (dev >= 0) ? dev : -1 * dev;
            dev_global += dev;
        }
        dev_global = dev_global / ((float)(l_end - l_begin + 1));

        /* global threshold */
        float T_global = ave_global + alpha1 * dev_global;

        /* get local maximum */
        int localmaxpos = s_begin;
        for (int i = s_begin; i <= s_end; i++) {
            if (dist[i] > dist[localmaxpos]) localmaxpos = i;
        }
        /* get 2nd local maximum */
        int localmaxpos2 = s_begin;
        float localmax2 = 0;
        for (int i = s_begin; i <= s_end; i++) {
            if (i == localmaxpos) continue;
            if (dist[i] > localmax2) {
                localmaxpos2 = i;
                localmax2 = dist[i];
            }
        }
        float T_local = alpha2 * dist[localmaxpos2];
        float Thresh = (T_global >= T_local) ? T_global : T_local;

        if ((dist[k] == dist[localmaxpos]) && (dist[k] > Thresh)) {
            bnds[k] = 1;
            nbboundaries++;
        } else {
            bnds[k] = 0;
        }
        k++;
    } while (k < nbframes - 1);
    bnds[nbframes - 1] = 1;
    nbboundaries += 2;

    int start = 0;
    int end = 0;
    int nbselectedframes = 0;
    do {
        /* find next boundary */
        do {
            end++;
        } while ((bnds[end] != 1) && (end < nbframes));

        /* find min disparity within bounds */
        int minpos = start + 1;
        for (int i = start + 1; i < end; i++) {
            if (dist[i] < dist[minpos]) minpos = i;
        }
        bnds[minpos] = 2;
        nbselectedframes++;
        start = end;
    } while (start < nbframes - 1);

    st_info.nb_retrieval = 1;
    st_info.width = 32;
    st_info.height = 32;
    k = 0;
    do {
        if (bnds[k] == 2) {
            if (ReadFrames(&st_info, pframelist, k * st_info.step,
                           k * st_info.step + 1) < 0) {
                delete pframelist;
                free(dist);
                return NULL;
            }
        }
        k++;
    } while (k < nbframes);
    vfinfo_close(&st_info);

    free(bnds);
    bnds = NULL;
    free(dist);
    dist = NULL;

    return pframelist;
}

ulong64 *ph_dct_videohash(const char *filename, int &Length) {
    Length = 0;
    CImgList<uint8_t> *keyframes = ph_getKeyFramesFromVideo(filename);
    if (keyframes == NULL) return NULL;
    if (keyframes->size() == 0) {
        delete keyframes;
        return NULL;
    }

    Length = keyframes->size();

    ulong64 *hash = (ulong64 *)malloc(sizeof(ulong64) * Length);
    if (hash == NULL) {
        keyframes->clear();
        delete keyframes;
        Length = 0;
        return NULL;
    }
    const CImg<float> &C = get_dct_matrix();
    CImg<float> Ctransp = C.get_transpose();
    CImg<float> dctImage;
    CImg<float> subsec;
    CImg<uint8_t> currentframe;

    for (unsigned int i = 0; i < keyframes->size(); i++) {
        currentframe = keyframes->at(i);
        currentframe.blur(1.0);
        dctImage = C * currentframe * Ctransp;
        // Match ph_dct_imagehash: canonical top-left 8x8 of the DCT,
        // median over the 63 AC coefficients (skip DC at position 0).
        subsec = dctImage.crop(0, 0, 7, 7).unroll('x');
        CImg<float> ac = subsec.get_crop(1, 0, 0, 0, 63, 0, 0, 0);
        float med = ac.median();
        hash[i] = 0x0000000000000000;
        ulong64 one = 0x0000000000000001;
        for (int j = 0; j < 64; j++) {
            if (subsec(j) > med) hash[i] |= one;
            one = one << 1;
        }
    }

    keyframes->clear();
    delete keyframes;
    keyframes = NULL;

    return hash;
}

double ph_dct_videohash_dist(ulong64 *hashA, int N1, ulong64 *hashB, int N2,
                             int threshold) {
    if (hashA == NULL || hashB == NULL || N1 <= 0 || N2 <= 0) return 0.0;

    int den = (N1 <= N2) ? N1 : N2;

    // Rolling LCS: only need the previous and current rows. Avoids the
    // (N1+1)*(N2+1) VLA, which was a stack overflow waiting to happen.
    int *prev = (int *)calloc((size_t)N2 + 1, sizeof(int));
    int *curr = (int *)calloc((size_t)N2 + 1, sizeof(int));
    if (!prev || !curr) {
        free(prev);
        free(curr);
        return 0.0;
    }

    for (int i = 1; i < N1 + 1; i++) {
        curr[0] = 0;
        for (int j = 1; j < N2 + 1; j++) {
            int d = ph_hamming_distance(hashA[i - 1], hashB[j - 1]);
            if (d <= threshold) {
                curr[j] = prev[j - 1] + 1;
            } else {
                curr[j] = (prev[j] >= curr[j - 1]) ? prev[j] : curr[j - 1];
            }
        }
        int *tmp = prev;
        prev = curr;
        curr = tmp;
    }

    double result = (double)(prev[N2]) / (double)(den);
    free(prev);
    free(curr);
    return result;
}

#endif

int ph_hamming_distance(const ulong64 hash1, const ulong64 hash2) {
    ulong64 x = hash1 ^ hash2;
    return __builtin_popcountll(x);
}

#ifdef HAVE_IMAGE_HASH

// Returns a Marr-Hildreth kernel for (alpha, level). The previous version
// memoised a single kernel with the *first* (alpha, level) ever requested,
// ignoring subsequent parameters; here each call returns a fresh kernel
// owned by the caller.
CImg<float> *GetMHKernel(float alpha, float level) {
    int sigma = (int)(4 * pow((float)alpha, (float)level));
    if (sigma <= 0) return NULL;
    CImg<float> *pkernel = NULL;
    try {
        pkernel = new CImg<float>(2 * sigma + 1, 2 * sigma + 1, 1, 1, 0);
    } catch (...) {
        return NULL;
    }
    cimg_forXY(*pkernel, X, Y) {
        float xpos = pow(alpha, -level) * (X - sigma);
        float ypos = pow(alpha, -level) * (Y - sigma);
        float A = xpos * xpos + ypos * ypos;
        pkernel->atXY(X, Y) = (2 - A) * exp(-A / 2);
    }
    return pkernel;
}

uint8_t *ph_mh_imagehash(const char *filename, int &N, float alpha, float lvl) {
    N = 0;
    if (filename == NULL) {
        return NULL;
    }

    CImg<uint8_t> src;
    try {
        if (load_image(filename, src) < 0) return NULL;
    } catch (CImgIOException &) {
        return NULL;
    } catch (CImgException &) {
        return NULL;
    }

    CImg<uint8_t> img;
    if (src.spectrum() == 3) {
        img = src.get_RGBtoYCbCr()
                  .channel(0)
                  .blur(1.0)
                  .resize(512, 512, 1, 1, 5)
                  .get_equalize(256);
    } else if (src.spectrum() > 3) {
        // RGBA (or wider): drop alpha and any extra channels before
        // RGBtoYCbCr, which requires exactly 3 channels. Without this,
        // a 4-channel PNG would throw CImgInstanceException inside this
        // function (the same issue #39 / PR #40 fixes in
        // _ph_image_digest).
        img = src.get_channels(0, 2)
                  .RGBtoYCbCr()
                  .channel(0)
                  .blur(1.0)
                  .resize(512, 512, 1, 1, 5)
                  .get_equalize(256);
    } else {
        img = src.get_channel(0)
                  .blur(1.0)
                  .resize(512, 512, 1, 1, 5)
                  .get_equalize(256);
    }
    src.clear();

    return ph_mh_imagehash_from_buffer(img, N, alpha, lvl);
}

uint8_t *ph_mh_imagehash_from_buffer(const CImg<uint8_t> &img, int &N,
                                     float alpha, float lvl) {
    N = 0;

    CImg<float> *pkernel = GetMHKernel(alpha, lvl);
    if (pkernel == NULL) return NULL;
    CImg<float> fresp = img.get_correlate(*pkernel);
    delete pkernel;

    uint8_t *hash = (uint8_t *)malloc(72 * sizeof(uint8_t));
    if (hash == NULL) return NULL;
    N = 72;
    fresp.normalize(0, 1.0);
    CImg<float> blocks(31, 31, 1, 1, 0);
    for (int rindex = 0; rindex < 31; rindex++) {
        for (int cindex = 0; cindex < 31; cindex++) {
            blocks(rindex, cindex) =
                fresp
                    .get_crop(rindex * 16, cindex * 16, rindex * 16 + 16 - 1,
                              cindex * 16 + 16 - 1)
                    .sum();
        }
    }
    int hash_index;
    int nb_ones = 0, nb_zeros = 0;
    int bit_index = 0;
    unsigned char hashbyte = 0;
    for (int rindex = 0; rindex < 31 - 2; rindex += 4) {
        CImg<float> subsec;
        for (int cindex = 0; cindex < 31 - 2; cindex += 4) {
            subsec = blocks.get_crop(cindex, rindex, cindex + 2, rindex + 2)
                         .unroll('x');
            float ave = subsec.mean();
            cimg_forX(subsec, I) {
                hashbyte <<= 1;
                if (subsec(I) > ave) {
                    hashbyte |= 0x01;
                    nb_ones++;
                } else {
                    nb_zeros++;
                }
                bit_index++;
                if ((bit_index % 8) == 0) {
                    hash_index = (int)(bit_index / 8) - 1;
                    hash[hash_index] = hashbyte;
                    hashbyte = 0x00;
                }
            }
        }
    }

    return hash;
}

// C-callable wrapper: build a CImg from a raw interleaved pixel buffer,
// apply the same preprocessing as the file-based ph_mh_imagehash
// (RGBtoYCbCr -> luminance -> blur -> resize 512x512 -> equalize),
// then delegate to ph_mh_imagehash_from_buffer.
//
// Exists so language bindings (Java JNI, C# P/Invoke, etc.) can hash an
// image they hold in memory without round-tripping through a temp file.
// Layout expected:
//   - pixels: interleaved, row-major; for channels=3 this is RGB,RGB,RGB...
//     for channels=4 it is RGBA,RGBA,...; for channels=1 it is luminance.
//   - width / height: pixel dimensions
//   - channels: 1, 3, or 4 (4-channel input is treated as RGB with the alpha
//     channel discarded, matching ph_dct_imagehash's handling)
uint8_t *ph_mh_imagehash_from_pixels(const uint8_t *pixels, int width,
                                     int height, int channels, int *N,
                                     float alpha, float lvl) {
    if (N == NULL) return NULL;
    *N = 0;
    if (pixels == NULL || width <= 0 || height <= 0) return NULL;
    if (channels != 1 && channels != 3 && channels != 4) return NULL;

    CImg<uint8_t> src;
    try {
        // Interleaved -> planar conversion (CImg stores planar by default).
        // Construct with (channels, width, height) so channels is the
        // fastest-varying dimension matching the input layout, then permute
        // to put channels on the proper c axis. Same trick used in
        // cimgffmpeg.cpp's ReadFrames.
        CImg<uint8_t> interleaved(pixels, channels, width, height, 1, false);
        src = interleaved.get_permute_axes("yzcx");
    } catch (...) {
        return NULL;
    }

    CImg<uint8_t> img;
    try {
        if (src.spectrum() >= 3) {
            // RGB or RGBA: drop any alpha, take Y from YCbCr.
            img = src.get_channels(0, 2)
                      .RGBtoYCbCr()
                      .channel(0)
                      .blur(1.0)
                      .resize(512, 512, 1, 1, 5)
                      .get_equalize(256);
        } else {
            img = src.get_channel(0)
                      .blur(1.0)
                      .resize(512, 512, 1, 1, 5)
                      .get_equalize(256);
        }
    } catch (...) {
        return NULL;
    }

    return ph_mh_imagehash_from_buffer(img, *N, alpha, lvl);
}
#endif

int ph_bitcount8(uint8_t val) {
    int num = 0;
    while (val) {
        ++num;
        val &= val - 1;
    }
    return num;
}

double ph_hammingdistance2(uint8_t *hashA, int lenA, uint8_t *hashB, int lenB) {
    if (lenA != lenB) {
        return -1.0;
    }
    if ((hashA == NULL) || (hashB == NULL) || (lenA <= 0)) {
        return -1.0;
    }
    double dist = 0;
    uint8_t D = 0;
    for (int i = 0; i < lenA; i++) {
        D = hashA[i] ^ hashB[i];
        dist = dist + (double)ph_bitcount8(D);
    }
    double bits = (double)lenA * 8;
    return dist / bits;
}

// Returns true if d is one of the characters fed into the rolling hash
// (digit or lowercase letter), normalising the case in-place.
static bool is_text_char(int &d) {
    if (d <= 47) return false;
    if ((d >= 58 && d <= 64) || (d >= 91 && d <= 96) || d >= 123) return false;
    if (d >= 65 && d <= 90) d += 32;
    return true;
}

TxtHashPoint *ph_texthash(const char *filename, int *nbpoints) {
    if (filename == NULL || nbpoints == NULL) return NULL;
    *nbpoints = 0;

    unsigned char kgram[KgramLength];

    FILE *pfile = fopen(filename, "r");
    if (!pfile) {
        return NULL;
    }

    // Growable output buffer (doubled on overflow). Start with a small
    // estimate based on Schleimer's expected density of 2/(w+1) selected
    // fingerprints per k-gram (Lemma 3.1 of the winnowing paper); the
    // worst case is one per k-gram but that's rare on real text.
    struct stat fileinfo;
    if (fstat(fileno(pfile), &fileinfo) != 0 || fileinfo.st_size <= 0) {
        fclose(pfile);
        return NULL;
    }
    size_t capacity = (size_t)fileinfo.st_size / WindowLength * 4 + 64;
    TxtHashPoint *TxtHash =
        (TxtHashPoint *)malloc(capacity * sizeof(struct ph_hash_point));
    if (!TxtHash) {
        fclose(pfile);
        return NULL;
    }

    int d;
    ulong64 hashword = 0ULL;
    int first = 0, last = KgramLength - 1;
    int text_index = 0;
    int valid_count = 0;

    // Fill the first kgram with KgramLength *valid* characters.
    while (valid_count < KgramLength) {
        d = fgetc(pfile);
        if (d == EOF) {
            free(TxtHash);
            fclose(pfile);
            return NULL;
        }
        text_index++;
        if (!is_text_char(d)) continue;

        kgram[valid_count++] = (unsigned char)d;
        hashword = hashword << delta;
        hashword = hashword ^ textkeys[(unsigned char)d];
    }

    // Sliding-window winnowing per Schleimer/Wilkerson/Aiken 2003.
    // The previous implementation used a tumbling window (jumping the
    // window by WindowLength k-grams each emit), which loses the paper's
    // matching guarantee: any duplicate substring of length >= k + w - 1
    // is supposed to produce at least one identical fingerprint, but with
    // tumbling windows a duplicate straddling a window boundary can be
    // missed.
    //
    // We maintain a monotonic deque of (hash, text_index, kgram_pos)
    // entries with hashes strictly increasing from front to back. The
    // front is therefore the minimum hash in the current window. On each
    // new k-gram we (a) pop back entries with hash >= new hash (so a
    // later equal value replaces an earlier one, giving the right-most
    // minimum on ties as the paper specifies), (b) push the new entry,
    // (c) drop the front if it has slid out of the window, (d) emit the
    // current front if its k-gram position differs from the last emitted.
    struct deque_entry {
        ulong64 hash;
        off_t text_index;
        int kgram_pos;
    };
    std::vector<deque_entry> dq;
    dq.reserve((size_t)WindowLength);

    int kgram_pos = 0;
    int prev_emitted_pos = -1;
    long long emits = 0;

    auto push_kgram = [&](ulong64 h, off_t ti) {
        // (a) maintain monotonic increasing back -> front
        while (!dq.empty() && dq.back().hash >= h) dq.pop_back();
        // (b) append
        deque_entry e = {h, ti, kgram_pos};
        dq.push_back(e);
        // (c) drop entries that fell out of the window
        while (!dq.empty() && dq.front().kgram_pos <= kgram_pos - WindowLength) {
            dq.erase(dq.begin());
        }
    };

    push_kgram(hashword, text_index);
    // The first window completes at kgram_pos = WindowLength - 1.

    while ((d = fgetc(pfile)) != EOF) {
        text_index++;
        if (!is_text_char(d)) continue;

        ulong64 oldsym = textkeys[kgram[first % KgramLength]];
        oldsym = oldsym << (delta * KgramLength);
        hashword = hashword << delta;
        hashword = hashword ^ textkeys[(unsigned char)d];
        hashword = hashword ^ oldsym;
        kgram[last % KgramLength] = (unsigned char)d;
        first++;
        last++;
        kgram_pos++;

        push_kgram(hashword, text_index);

        // (d) emit if the window is full and the current min position
        // differs from the last emitted.
        if (kgram_pos >= WindowLength - 1) {
            const deque_entry &m = dq.front();
            if (m.kgram_pos != prev_emitted_pos) {
                if ((size_t)emits >= capacity) {
                    size_t new_cap = capacity * 2;
                    TxtHashPoint *tmp = (TxtHashPoint *)realloc(
                        TxtHash, new_cap * sizeof(struct ph_hash_point));
                    if (!tmp) {
                        free(TxtHash);
                        fclose(pfile);
                        *nbpoints = 0;
                        return NULL;
                    }
                    TxtHash = tmp;
                    capacity = new_cap;
                }
                TxtHash[emits].hash = m.hash;
                TxtHash[emits].index = m.text_index;
                emits++;
                prev_emitted_pos = m.kgram_pos;
            }
        }
    }

    *nbpoints = (int)emits;
    fclose(pfile);
    return TxtHash;
}

TxtMatch *ph_compare_text_hashes(TxtHashPoint *hash1, int N1,
                                 TxtHashPoint *hash2, int N2,
                                 int *nbmatches) {
    if (nbmatches == NULL) return NULL;
    *nbmatches = 0;
    if (hash1 == NULL || hash2 == NULL || N1 <= 0 || N2 <= 0) return NULL;

    int max_matches = (N1 >= N2) ? N1 : N2;
    TxtMatch *found_matches =
        (TxtMatch *)malloc((size_t)max_matches * sizeof(TxtMatch));
    if (!found_matches) {
        return NULL;
    }

    // Build a hash -> vector<index> index over hash2 so the outer scan over
    // hash1 only inspects matching positions. Reduces the worst case from
    // O(N1*N2*min(N1,N2)) to O((N1+N2)*L) where L is the average extension
    // length.
    std::map<ulong64, std::vector<int>> hash2_index;
    for (int j = 0; j < N2; j++) {
        hash2_index[hash2[j].hash].push_back(j);
    }

    for (int i = 0; i < N1; i++) {
        std::map<ulong64, std::vector<int>>::const_iterator it =
            hash2_index.find(hash1[i].hash);
        if (it == hash2_index.end()) continue;
        for (size_t k = 0; k < it->second.size(); k++) {
            int j = it->second[k];
            int m = i + 1;
            int n = j + 1;
            int cnt = 1;
            while (m < N1 && n < N2 && hash1[m].hash == hash2[n].hash) {
                cnt++;
                m++;
                n++;
            }
            if (*nbmatches >= max_matches) goto done;
            found_matches[*nbmatches].first_index = i;
            found_matches[*nbmatches].second_index = j;
            found_matches[*nbmatches].length = cnt;
            (*nbmatches)++;
        }
    }
done:
    return found_matches;
}
