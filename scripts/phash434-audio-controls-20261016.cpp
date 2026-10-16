#include "audiophash.h"
#include <cstdio>
#include <string>
#include <vector>
#include <fcntl.h>

static int descriptors() {
    int count = 0;
    for (int fd = 0; fd < 4096; ++fd) if (fcntl(fd, F_GETFD) != -1) ++count;
    return count;
}

int main(int argc, char** argv) {
    if (argc < 2) return 2;
    std::string mode(argv[1]);
    if (mode == "short-block") {
        uint32_t a[1] = {0}, b[1] = {0};
        int count = -1;
        double* result = ph_audio_distance_ber(a,1,b,1,0.25f,2,count);
        delete[] result;
        return 3;
    }
    if (mode == "zero-block") {
        uint32_t a[1] = {0}, b[1] = {0};
        int count = -1;
        double* result = ph_audio_distance_ber(a,1,b,1,0.25f,0,count);
        delete[] result;
        return 3;
    }
    if (mode == "short-hash") {
        float a[1] = {0};
        int count = -1;
        uint32_t* result = ph_audiohash(a,0,8000,count);
        std::free(result);
        return 3;
    }
    if (mode != "positive" || argc != 3) return 2;
    unsigned checked = 0;
    for (unsigned a = 0; a < 256; ++a) for (unsigned b = 0; b < 256; ++b) {
        uint32_t left = a, right = b;
        double reference = __builtin_popcount(a ^ b)/32.0;
        if (ph_compare_blocks(&left,&right,1) != reference) return 4;
        ++checked;
    }
    for (unsigned bit = 0; bit < 32; ++bit)
        if (ph_bitcount(uint32_t(1) << bit) != 1) return 5;
    for (int length : {4096,4224,4352}) {
        std::vector<float> signal(length,0);
        int frames = -1;
        uint32_t* hashes = ph_audiohash(signal.data(),length,8000,frames);
        if (!hashes || frames != (length-4096)/128+1) return 6;
        for (int i = 0; i < frames; ++i) if (hashes[i] != 0) return 7;
        std::free(hashes);
    }
    int before = descriptors();
    for (int i = 0; i < 16; ++i) {
        int length = -1;
        float* signal = ph_readaudio(argv[2],8000,1,nullptr,length,0);
        if (!signal || length != 800) return 8;
        std::free(signal);
    }
    int leaked = descriptors() - before;
    std::printf("block_oracle_passes=%u single_bits=32 silent_hashes=3 descriptor_growth=%d\n", checked, leaked);
    return leaked == 16 ? 0 : 9;
}
