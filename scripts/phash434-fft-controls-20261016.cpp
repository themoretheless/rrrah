#include "ph_fft.h"
#include <cmath>
#include <cstdio>
#include <vector>
#include <string>

int main(int argc, char** argv) {
    if (argc != 2) return 2;
    if (std::string(argv[1]) == "zero") {
        double input[1] = {0};
        std::complex<double> output[1];
        return fft(input, 0, output);
    }
    unsigned correct = 0, odd_failures = 0;
    for (int length : {1,2,4,8,16,32,64,128}) {
        for (int seed = 0; seed < 8; ++seed) {
            std::vector<double> input(length);
            std::vector<std::complex<double>> output(length);
            for (int i = 0; i < length; ++i) input[i] = std::sin((i+1)*(seed+1)*0.31);
            if (fft(input.data(), length, output.data()) != 0) return 3;
            for (int k = 0; k < length; ++k) {
                std::complex<double> reference(0,0);
                for (int i = 0; i < length; ++i)
                    reference += input[i] * std::polar(1.0, 2*std::acos(-1.0)*k*i/length);
                if (std::abs(reference-output[k]) > 1e-9) return 4;
            }
            ++correct;
        }
    }
    for (int length : {3,5,7,9,15,17}) {
        std::vector<double> input(length, 1.0);
        std::vector<std::complex<double>> output(length, {12345.0,67890.0});
        int code = fft(input.data(), length, output.data());
        bool correct = true;
        for (int k = 0; k < length; ++k) {
            std::complex<double> reference(0,0);
            for (int i = 0; i < length; ++i)
                reference += input[i] * std::polar(1.0, 2*std::acos(-1.0)*k*i/length);
            if (std::abs(reference-output[k]) > 1e-9) correct = false;
        }
        if (code == 0 && !correct) ++odd_failures;
    }
    std::printf("power_two_oracle_passes=%u odd_lengths_silently_wrong=%u\n", correct, odd_failures);
    return correct == 64 && odd_failures == 6 ? 0 : 5;
}
