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
    David Starkweather - dstarkweather@phash.org

*/

#include "audiophash.h"

#include <samplerate.h>
#include <sndfile.h>

#include <vector>
#ifdef HAVE_LIBMPG123
#include <mpg123.h>
#endif

int ph_count_samples(const char *filename, int /*sr*/, int /*channels*/) {
    SF_INFO sf_info;
    sf_info.format = 0;
    SNDFILE *sndfile = sf_open(filename, SFM_READ, &sf_info);
    if (sndfile == NULL) {
        return -1;
    }
    int count = (int)sf_info.frames;
    sf_close(sndfile);
    return count;
}

#ifdef HAVE_LIBMPG123

static float *readaudio_mp3(const char *filename, long *sr, const float nbsecs,
                            unsigned int *buflen) {
    mpg123_handle *m = NULL;
    unsigned char *decbuf = NULL;
    float *buffer = NULL;
    bool mpg_inited = false;
    int ret;

    *buflen = 0;

    if (mpg123_init() != MPG123_OK) {
        fprintf(stderr, "unable to init mpg\n");
        return NULL;
    }
    mpg_inited = true;

    m = mpg123_new(NULL, &ret);
    if (m == NULL) {
        fprintf(stderr, "unable to init mpg\n");
        goto fail;
    }

    if (mpg123_open(m, filename) != MPG123_OK) {
        fprintf(stderr, "unable to open mp3\n");
        goto fail;
    }

    mpg123_param(m, MPG123_ADD_FLAGS, MPG123_QUIET, 0);
    mpg123_scan(m);

    {
        off_t totalsamples = mpg123_length(m);
        if (totalsamples <= 0) goto fail;

        int channels = 0, encoding = 0;
        if (mpg123_getformat(m, sr, &channels, &encoding) != MPG123_OK) {
            fprintf(stderr, "unable to get format\n");
            goto fail;
        }
        if (channels <= 0 || *sr <= 0) goto fail;

        mpg123_format_none(m);
        if (mpg123_format(m, *sr, channels, encoding) != MPG123_OK) goto fail;

        size_t decbuflen = mpg123_outblock(m);
        decbuf = (unsigned char *)malloc(decbuflen);
        if (decbuf == NULL) {
            fprintf(stderr, "mem alloc error\n");
            goto fail;
        }

        unsigned int nbsamples =
            (nbsecs <= 0) ? (unsigned int)totalsamples
                          : (unsigned int)(nbsecs * (*sr));
        if ((off_t)nbsamples > totalsamples) {
            nbsamples = (unsigned int)totalsamples;
        }
        if (nbsamples == 0) goto fail;

        buffer = (float *)malloc((size_t)nbsamples * sizeof(float));
        if (buffer == NULL) goto fail;
        *buflen = nbsamples;

        size_t i, j, index = 0, done = 0;
        do {
            ret = mpg123_read(m, decbuf, decbuflen, &done);
            switch (encoding) {
                case MPG123_ENC_SIGNED_16: {
                    size_t samples = done / sizeof(short);
                    if (samples < (size_t)channels) break;
                    size_t limit = samples - (size_t)channels + 1;
                    for (i = 0; i < limit; i += channels) {
                        float acc = 0.0f;
                        for (j = 0; j < (size_t)channels; j++) {
                            acc += (float)(((short *)decbuf)[i + j]) /
                                   (float)SHRT_MAX;
                        }
                        buffer[index++] = acc / channels;
                        if (index >= nbsamples) break;
                    }
                } break;
                case MPG123_ENC_SIGNED_8: {
                    size_t samples = done / sizeof(char);
                    if (samples < (size_t)channels) break;
                    size_t limit = samples - (size_t)channels + 1;
                    for (i = 0; i < limit; i += channels) {
                        float acc = 0.0f;
                        for (j = 0; j < (size_t)channels; j++) {
                            acc += (float)(((char *)decbuf)[i + j]) /
                                   (float)SCHAR_MAX;
                        }
                        buffer[index++] = acc / channels;
                        if (index >= nbsamples) break;
                    }
                } break;
                case MPG123_ENC_FLOAT_32: {
                    size_t samples = done / sizeof(float);
                    if (samples < (size_t)channels) break;
                    size_t limit = samples - (size_t)channels + 1;
                    for (i = 0; i < limit; i += channels) {
                        float acc = 0.0f;
                        for (j = 0; j < (size_t)channels; j++) {
                            acc += ((float *)decbuf)[i + j];
                        }
                        buffer[index++] = acc / channels;
                        if (index >= nbsamples) break;
                    }
                } break;
                default:
                    done = 0;
            }
        } while (ret == MPG123_OK && index < nbsamples);

        if (index == 0) {
            free(buffer);
            buffer = NULL;
            *buflen = 0;
            goto fail;
        }
        *buflen = (unsigned int)index;
    }

    free(decbuf);
    mpg123_close(m);
    mpg123_delete(m);
    mpg123_exit();
    return buffer;

fail:
    free(decbuf);
    if (m) {
        mpg123_close(m);
        mpg123_delete(m);
    }
    if (mpg_inited) mpg123_exit();
    return buffer ? buffer : NULL;
}

#endif /*HAVE_LIBMPG123*/

static float *readaudio_snd(const char *filename, long *sr, const float nbsecs,
                            unsigned int *buflen) {
    *buflen = 0;
    SF_INFO sf_info;
    sf_info.format = 0;
    SNDFILE *sndfile = sf_open(filename, SFM_READ, &sf_info);
    if (sndfile == NULL) {
        return NULL;
    }
    if (sf_info.channels <= 0 || sf_info.samplerate <= 0 ||
        sf_info.frames <= 0) {
        sf_close(sndfile);
        return NULL;
    }

    sf_command(sndfile, SFC_SET_NORM_FLOAT, NULL, SF_TRUE);

    *sr = (long)sf_info.samplerate;

    unsigned int src_frames =
        (nbsecs <= 0) ? (unsigned int)sf_info.frames
                      : (unsigned int)(nbsecs * sf_info.samplerate);
    if ((sf_count_t)src_frames > sf_info.frames) {
        src_frames = (unsigned int)sf_info.frames;
    }
    if (src_frames == 0) {
        sf_close(sndfile);
        return NULL;
    }

    float *inbuf = (float *)malloc((size_t)src_frames *
                                   (size_t)sf_info.channels * sizeof(float));
    if (inbuf == NULL) {
        sf_close(sndfile);
        return NULL;
    }

    sf_count_t cnt_frames = sf_readf_float(sndfile, inbuf, src_frames);
    sf_close(sndfile);
    if (cnt_frames <= 0) {
        free(inbuf);
        return NULL;
    }

    float *buf = (float *)malloc((size_t)cnt_frames * sizeof(float));
    if (buf == NULL) {
        free(inbuf);
        return NULL;
    }

    int indx = 0;
    for (sf_count_t i = 0; i < cnt_frames * sf_info.channels;
         i += sf_info.channels) {
        float acc = 0.0f;
        for (int j = 0; j < sf_info.channels; j++) {
            acc += inbuf[i + j];
        }
        buf[indx++] = acc / sf_info.channels;
    }
    free(inbuf);

    *buflen = (unsigned int)indx;
    return buf;
}

float *ph_readaudio2(const char *filename, int sr, float * /*sigbuf*/,
                     int &buflen, const float nbsecs) {
    buflen = 0;
    if (filename == NULL || sr <= 0) return NULL;

    long orig_sr = 0;
    float *inbuffer = NULL;
    unsigned int inbufferlength = 0;

    const char *suffix = strrchr(filename, '.');
    if (suffix == NULL) return NULL;
    if (!strcasecmp(suffix + 1, "mp3")) {
#ifdef HAVE_LIBMPG123
        inbuffer = readaudio_mp3(filename, &orig_sr, nbsecs, &inbufferlength);
#endif
    } else {
        inbuffer = readaudio_snd(filename, &orig_sr, nbsecs, &inbufferlength);
    }

    if (inbuffer == NULL || inbufferlength == 0 || orig_sr <= 0) {
        free(inbuffer);
        return NULL;
    }

    double sr_ratio = (double)(sr) / (double)orig_sr;
    if (src_is_valid_ratio(sr_ratio) == 0) {
        free(inbuffer);
        return NULL;
    }

    unsigned int outbufferlength =
        (unsigned int)(sr_ratio * (double)inbufferlength) + 1;
    float *outbuffer =
        (float *)malloc((size_t)outbufferlength * sizeof(float));
    if (!outbuffer) {
        free(inbuffer);
        return NULL;
    }

    int error;
    SRC_STATE *src_state = src_new(SRC_LINEAR, 1, &error);
    if (!src_state) {
        free(inbuffer);
        free(outbuffer);
        return NULL;
    }

    SRC_DATA src_data;
    src_data.data_in = inbuffer;
    src_data.data_out = outbuffer;
    src_data.input_frames = inbufferlength;
    src_data.output_frames = outbufferlength;
    src_data.end_of_input = SF_TRUE;
    src_data.src_ratio = sr_ratio;

    if ((error = src_process(src_state, &src_data)) != 0) {
        free(inbuffer);
        free(outbuffer);
        src_delete(src_state);
        return NULL;
    }

    // libsamplerate fills output_frames_gen with the count actually produced;
    // output_frames is the capacity the caller supplied. Using the latter
    // (the original bug) over-reads outbuffer downstream.
    buflen = (int)src_data.output_frames_gen;

    src_delete(src_state);
    free(inbuffer);

    if (buflen <= 0) {
        free(outbuffer);
        return NULL;
    }

    return outbuffer;
}

float *ph_readaudio(const char *filename, int sr, int /*channels*/,
                    float *sigbuf, int &buflen, const float nbsecs) {
    if (!filename || sr <= 0) return NULL;
    return ph_readaudio2(filename, sr, sigbuf, buflen, nbsecs);
}

// Round target up to the nearest power of 2, then choose whichever of
// {that, that/2} is closer to target. Used to size FFT frames given a
// target duration. The custom radix-2 FFT in ph_fft requires a power
// of 2.
static int closest_pow2(double target) {
    if (target <= 1.0) return 1;
    int p = 1;
    while ((double)p < target) p *= 2;
    int prev = p / 2;
    return (target - (double)prev) > ((double)p - target) ? p : prev;
}

uint32_t *ph_audiohash(float *buf, int N, int sr, int &nb_frames) {
    nb_frames = 0;
    if (buf == NULL || sr <= 0) return NULL;

    // Haitsma-Kalker parameters: 0.37 s frames, 31.25 frames/sec
    // (i.e. ~32 ms advance). Frame size is rounded to a power of 2
    // because the bundled FFT is radix-2.
    const int frame_length = closest_pow2((double)sr * 0.37);
    const int nfft = frame_length;
    const int nfft_half = frame_length / 2;
    if (N < frame_length) return NULL;

    int advance = (int)std::round((double)sr / 31.25);
    if (advance <= 0) advance = 1;

    nb_frames = (N - frame_length) / advance + 1;
    if (nb_frames <= 0) {
        nb_frames = 0;
        return NULL;
    }

    std::vector<double> window(frame_length);
    for (int i = 0; i < frame_length; i++) {
        window[i] = 0.54 - 0.46 * cos(2 * M_PI * i / (frame_length - 1));
    }

    std::vector<double> frame(frame_length);
    std::vector<std::complex<double>> pF(nfft);
    std::vector<double> magnF(nfft_half);

    // Haitsma-Kalker: 33 log-spaced bands from 300 to 2000 Hz. The
    // previous value of 3000 Hz extended the upper band by 1 kHz beyond
    // the paper.
    const double minfreq = 300;
    const double maxfreq = 2000;
    const double minbark = 6 * asinh(minfreq / 600.0);
    const double maxbark = 6 * asinh(maxfreq / 600.0);
    const double nyqbark = maxbark - minbark;
    const int nfilts = 33;
    const double stepbarks = nyqbark / (nfilts - 1);
    const int nb_barks = nfft_half / 2 + 1;
    const double barkwidth = 1.06;

    std::vector<double> binbarks(nb_barks);
    std::vector<double> curr_bark(nfilts, 0.0);
    std::vector<double> prev_bark(nfilts, 0.0);
    for (int i = 0; i < nb_barks; i++) {
        binbarks[i] = 6 * asinh((double)i * sr / nfft_half / 600.0);
    }

    // wts[i*nfft_half + j]
    std::vector<double> wts((size_t)nfilts * (size_t)nfft_half, 0.0);
    for (int i = 0; i < nfilts; i++) {
        double f_bark_mid = minbark + i * stepbarks;
        for (int j = 0; j < nb_barks; j++) {
            double barkdiff = binbarks[j] - f_bark_mid;
            double lof = -2.5 * (barkdiff / barkwidth - 0.5);
            double hif = barkdiff / barkwidth + 0.5;
            double m = std::min(lof, hif);
            m = std::min(0.0, m);
            m = pow(10, m);
            wts[(size_t)i * nfft_half + j] = m;
        }
    }

    uint32_t *hash = (uint32_t *)malloc((size_t)nb_frames * sizeof(uint32_t));
    if (hash == NULL) {
        nb_frames = 0;
        return NULL;
    }

    int start = 0;
    int end = start + frame_length - 1;
    int index = 0;

    while (end < N && index < nb_frames) {
        double maxF = 0.0;
        double maxB = 0.0;
        for (int i = 0; i < frame_length; i++) {
            frame[i] = window[i] * buf[start + i];
        }
        if (fft(frame.data(), frame_length, pF.data()) < 0) {
            free(hash);
            nb_frames = 0;
            return NULL;
        }
        for (int i = 0; i < nfft_half; i++) {
            magnF[i] = std::abs(pF[i]);
            if (magnF[i] > maxF) maxF = magnF[i];
        }

        for (int i = 0; i < nfilts; i++) {
            curr_bark[i] = 0;
            const double *wts_row = &wts[(size_t)i * nfft_half];
            for (int j = 0; j < nfft_half; j++) {
                curr_bark[i] += wts_row[j] * magnF[j];
            }
            if (curr_bark[i] > maxB) maxB = curr_bark[i];
        }

        uint32_t curr_hash = 0x00000000u;
        for (int m = 0; m < nfilts - 1; m++) {
            double H = curr_bark[m] - curr_bark[m + 1] -
                       (prev_bark[m] - prev_bark[m + 1]);
            curr_hash = curr_hash << 1;
            if (H > 0) curr_hash |= 0x00000001;
        }

        hash[index++] = curr_hash;
        for (int i = 0; i < nfilts; i++) prev_bark[i] = curr_bark[i];
        start += advance;
        end += advance;
    }

    nb_frames = index;
    return hash;
}

int ph_bitcount(uint32_t n) {
    return __builtin_popcount(n);
}

double ph_compare_blocks(const uint32_t *ptr_blockA, const uint32_t *ptr_blockB,
                         const int block_size) {
    if (block_size <= 0) return 1.0;
    double result = 0;
    for (int i = 0; i < block_size; i++) {
        uint32_t xordhash = ptr_blockA[i] ^ ptr_blockB[i];
        result += ph_bitcount(xordhash);
    }
    result = result / (32.0 * block_size);
    return result;
}

double *ph_audio_distance_ber(uint32_t *hash_a, const int Na, uint32_t *hash_b,
                              const int Nb, const float threshold,
                              const int block_size, int &Nc) {
    Nc = 0;
    if (hash_a == NULL || hash_b == NULL || Na <= 0 || Nb <= 0 ||
        block_size <= 0) {
        return NULL;
    }

    uint32_t *ptrA;
    uint32_t *ptrB;
    int N1, N2;
    if (Na <= Nb) {
        ptrA = hash_a;
        ptrB = hash_b;
        Nc = Nb - Na + 1;
        N1 = Na;
        N2 = Nb;
    } else {
        ptrB = hash_a;
        ptrA = hash_b;
        Nc = Na - Nb + 1;
        N1 = Nb;
        N2 = Na;
    }

    double *pC = (double *)malloc((size_t)Nc * sizeof(double));
    if (!pC) {
        Nc = 0;
        return NULL;
    }

    // Worst-case M occurs at i=0 (smallest constraint): floor(N1/block_size).
    // Allocate once outside the loop instead of realloc'ing per iteration.
    int max_M = N1 / block_size;
    if (max_M < 1) max_M = 1;
    double *dist = (double *)malloc((size_t)max_M * sizeof(double));
    if (!dist) {
        free(pC);
        Nc = 0;
        return NULL;
    }

    for (int i = 0; i < Nc; i++) {
        int M = std::min(N1, N2 - i) / block_size;
        if (M <= 0) {
            pC[i] = 0.5;
            continue;
        }

        uint32_t *pha = ptrA;
        uint32_t *phb = ptrB + i;
        int hash1_index = 0;
        int hash2_index = i;
        int k = 0;

        while (k < M && hash1_index + block_size <= N1 &&
               hash2_index + block_size <= N2) {
            dist[k++] = ph_compare_blocks(pha, phb, block_size);
            hash1_index += block_size;
            hash2_index += block_size;
            pha += block_size;
            phb += block_size;
        }

        if (k == 0) {
            pC[i] = 0.5;
            continue;
        }

        double sum_above = 0, sum_below = 0;
        for (int n = 0; n < k; n++) {
            if (dist[n] <= threshold) {
                sum_below += 1 - dist[n];
            } else {
                sum_above += 1 - dist[n];
            }
        }
        double above_factor = sum_above / k;
        double below_factor = sum_below / k;
        pC[i] = 0.5 * (1 + below_factor - above_factor);
    }

    free(dist);
    return pC;
}

/*
 * The previous version of this file contained a HAVE_PTHREAD-gated
 * ph_audio_thread / ph_audio_hashes pair that referenced types `slice` and
 * `DP` and a function `ph_num_threads()` that were never declared anywhere in
 * the public headers, so the block could not actually be compiled. The
 * slicing math was also off-by-one: for count=10, threads=3 it produced
 * (5, 3, 3) which walks one element past the input. The dead code has been
 * removed; callers can drive ph_readaudio / ph_audiohash from their own
 * thread pool.
 */
