// Independent qualification tool. Uses installed LibRaw, never linked into Rrrah.
// Build: c++ -std=c++17 -I/opt/homebrew/include scripts/raw-fixture-oracle.cpp \
//   -L/opt/homebrew/lib -lraw -o /tmp/rrrah-libraw-oracle
#include <libraw/libraw.h>
#include <fstream>
#include <iostream>
#include <iomanip>
#include <string>

static void quoted(std::ostream &out, const char *s) {
    out << '"';
    for (; *s; ++s) {
        unsigned char c = *s;
        if (c == '"' || c == '\\') out << '\\' << c;
        else if (c < 32) out << "\\u" << std::hex << std::setw(4) << std::setfill('0') << unsigned(c) << std::dec;
        else out << c;
    }
    out << '"';
}
// Optional Phase One pre-demosaic stages retain an independently processed
// full sensor. The default remains the original unpack oracle for all formats.
class FixtureRaw : public LibRaw {
    bool temporary = false;
public:
    int phaseone_stage(bool corrected) {
        if (!is_phaseone_compressed()) return LIBRAW_UNSPECIFIED_ERROR;
        raw2image_start();
        phase_one_allocate_tempbuffer();
        temporary = true;
        int status = phase_one_subtract_black(
            static_cast<unsigned short *>(imgdata.rawdata.raw_alloc), imgdata.rawdata.raw_image);
        return status == LIBRAW_SUCCESS && corrected ? phase_one_correct() : status;
    }
    ~FixtureRaw() { if (temporary) phase_one_free_tempbuffer(); }
};
int main(int argc, char **argv) {
    if (argc != 3 && argc != 4) { std::cerr << "usage: raw-fixture-oracle RAW OUTPUT_PREFIX [--phaseone-black|--phaseone-corrected]\n"; return 2; }
    const std::string stage = argc == 4 ? argv[3] : "unpack";
    if (stage != "unpack" && stage != "--phaseone-black" && stage != "--phaseone-corrected") return 2;
    FixtureRaw raw;
    int status = raw.open_file(argv[1]);
    if (status == LIBRAW_SUCCESS) status = raw.unpack();
    if (status == LIBRAW_SUCCESS && stage != "unpack") status = raw.phaseone_stage(stage == "--phaseone-corrected");
    if (status != LIBRAW_SUCCESS) { std::cerr << libraw_strerror(status) << '\n'; return 1; }
    const auto &d = raw.imgdata;
    if (!d.rawdata.raw_image) { std::cerr << "not a single-plane integer mosaic\n"; return 1; }
    std::ofstream pixels(std::string(argv[2])+".u16le", std::ios::binary);
    for (unsigned row=0; row<d.sizes.raw_height; ++row) {
        auto ptr = reinterpret_cast<const unsigned short *>(reinterpret_cast<const char *>(d.rawdata.raw_image)+row*d.sizes.raw_pitch);
        for (unsigned col=0; col<d.sizes.raw_width; ++col) {
            char bytes[2] = {char(ptr[col]&255), char(ptr[col]>>8)};
            pixels.write(bytes,2);
        }
    }
    std::ofstream out(std::string(argv[2])+".json");
    out << std::setprecision(10) << "{\"libraw_version\":";
    quoted(out, LibRaw::version());
    out << ",\"processing_stage\":"; quoted(out, stage.c_str());
    out << ",\"make\":"; quoted(out,d.idata.make);
    out << ",\"model\":"; quoted(out,d.idata.model);
    out << ",\"width\":" << d.sizes.raw_width << ",\"height\":" << d.sizes.raw_height;
    out << ",\"orientation_flip\":" << d.sizes.flip;
    out << ",\"crop\":[" << d.sizes.left_margin << ',' << d.sizes.top_margin << ',' << d.sizes.width << ',' << d.sizes.height << ']';
    out << ",\"wb\":[";
    for (unsigned i=0;i<4;++i) out << (i?",":"") << d.color.cam_mul[i];
    out << "],\"black\":" << d.color.black << ",\"cblack\":[";
    for (unsigned i=0;i<6;++i) out << (i?",":"") << d.color.cblack[i];
    out << "],\"white\":" << d.color.maximum << ",\"cfa\":[";
    // LibRaw COLOR takes active-image coordinates; query negative margins to
    // report CFA at full sensor (0,0), matching Rrrah's coordinate contract.
    for (int row=0;row<2;++row) for(int col=0;col<2;++col) {
        int index = raw.COLOR((row+8-int(d.sizes.top_margin%8))%8,(col+2-int(d.sizes.left_margin%2))%2);
        out << ((row||col)?",":"") << '"' << d.idata.cdesc[index] << '"';
    }
    out << "],\"cfa_indices\":[";
    for (int row=0;row<2;++row) for(int col=0;col<2;++col) {
        int index = raw.COLOR((row+8-int(d.sizes.top_margin%8))%8,(col+2-int(d.sizes.left_margin%2))%2);
        out << ((row||col)?",":"") << index;
    }
    out << "],\"xyz_to_camera\":[";
    for(unsigned row=0;row<4;++row) {
        out << (row?",":"") << '[';
        for(unsigned col=0;col<3;++col) out << (col?",":"") << d.color.cam_xyz[row][col];
        out << ']';
    }
    out << "],\"camera_to_rgb\":[";
    for(unsigned row=0;row<3;++row) {
        out << (row?",":"") << '[';
        for(unsigned col=0;col<3;++col) out << (col?",":"") << d.color.rgb_cam[row][col];
        out << ']';
    }
    out << "],\"camera_to_rgb4\":[";
    for(unsigned row=0;row<3;++row) {
        out << (row?",":"") << '[';
        for(unsigned col=0;col<4;++col) out << (col?",":"") << d.color.rgb_cam[row][col];
        out << ']';
    }
    // Preserve file-origin DNG calibration independently of the camera-table
    // cam_xyz above. These can differ legitimately and must not be conflated.
    out << "],\"file_color_matrices\":[";
    for (unsigned slot=0;slot<2;++slot) {
        const auto &color = d.color.dng_color[slot];
        out << (slot?",":"") << "{\"parsedfields\":" << color.parsedfields
            << ",\"illuminant\":" << color.illuminant << ",\"xyz_to_camera\":[";
        for (unsigned row=0;row<4;++row) {
            out << (row?",":"") << '[';
            for (unsigned col=0;col<3;++col) out << (col?",":"") << color.colormatrix[row][col];
            out << ']';
        }
        out << "]}";
    }
    out << "]}\n";
    if (!pixels || !out) return 1;
}
