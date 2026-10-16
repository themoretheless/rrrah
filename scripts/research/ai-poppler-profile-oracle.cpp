// Test-only oracle. Uses the installed Poppler library, never the production
// decoder. Bypasses only the CLI's erroneous perceptual OUTPUT support check.
// Build: c++ -std=c++20 this-file.cpp $(pkg-config --cflags --libs poppler lcms2)
// Args: input.pdf output.ppm profile.icc|- vector-aa:0|1
#include <memory>
#include <string>
#include <cstdio>
#include <cmath>
#include <lcms2.h>
#include <GlobalParams.h>
#include <PDFDoc.h>
#include <SplashOutputDev.h>
#include <splash/SplashBitmap.h>
#include <splash/SplashErrorCodes.h>
#include <goo/GooString.h>
int main(int argc, char **argv) {
    if (argc != 5 || (std::string(argv[4]) != "0" && std::string(argv[4]) != "1")) return 2;
    globalParams = std::make_unique<GlobalParams>();
    PDFDoc doc(std::make_unique<GooString>(argv[1]));
    if (!doc.isOk() || doc.getNumPages() < 1) return 3;
    SplashColor paper{};
    paper[0] = paper[1] = paper[2] = 255;
    SplashOutputDev output(splashModeRGB8, 4, paper, true);
    output.setFontAntialias(true);
    output.setVectorAntialias(std::string(argv[4]) == "1");
    if (std::string(argv[3]) != "-") {
        auto profile = make_GfxLCMSProfilePtr(cmsOpenProfileFromFile(argv[3], "r"));
        if (!profile || cmsGetColorSpace(profile.get()) != cmsSigCmykData ||
            !cmsIsIntentSupported(profile.get(), INTENT_PERCEPTUAL, LCMS_USED_AS_INPUT)) return 4;
        output.setDefaultCMYKProfile(profile);
        std::fprintf(stderr, "ORACLE_PROFILE perceptual_input_supported=1 profile_payload_unmodified=1\n");
    }
    output.startDoc(&doc);
    const double width = std::ceil(doc.getPageMediaWidth(1));
    const double height = std::ceil(doc.getPageMediaHeight(1));
    if (doc.getPageRotate(1) != 0 || !(width > 0. && width <= 4096. && height > 0. && height <= 4096.)) return 6;
    doc.displayPageSlice(&output, 1, 72., 72., 0, true, false, false, 0, 0, static_cast<int>(width), static_cast<int>(height));
    return output.getBitmap()->writePNMFile(argv[2]) == SplashError::NoError ? 0 : 5;
}
