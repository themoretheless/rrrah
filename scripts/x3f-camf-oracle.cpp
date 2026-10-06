// External test-only CAMF oracle; never linked into Rrrah.
#include <libraw/libraw.h>
#include <internal/x3f_tools.h>
#include <fstream>
#include <iostream>
int main(int argc, char **argv) {
    if (argc != 3) return 2;
    LibRaw_bigfile_datastream input(argv[1]);
    x3f_t *file = x3f_new_from_file(&input);
    if (!file) return 1;
    auto *entry = x3f_get_camf(file);
    if (!entry || x3f_load_data(file, entry) != X3F_OK) { x3f_delete(file); return 1; }
    auto &camf = entry->header.data_subsection.camf;
    std::ofstream out(argv[2], std::ios::binary);
    out.write(static_cast<const char *>(camf.decoded_data), camf.decoded_data_size);
    std::cout << LibRaw::version() << " CAMF bytes=" << camf.decoded_data_size << '\n';
    bool ok = bool(out);
    x3f_delete(file);
    return ok ? 0 : 1;
}
