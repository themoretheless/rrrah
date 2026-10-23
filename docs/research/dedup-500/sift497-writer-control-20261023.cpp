#include "src/hash/hash.hpp"
#include <iostream>
#include <utility>
std::string HashResult::to_hex() const {
    static const char hex_chars[] = "0123456789abcdef";
    std::string out;
    out.reserve(bits.size() * 2);
    for (uint8_t b : bits) {
        out.push_back(hex_chars[(b >> 4) & 0xF]);
        out.push_back(hex_chars[b & 0xF]);
    }
    return out;
}

static void write_hash_json(
    std::ostream& out,
    const std::string& algo,
    int size,
    const std::vector<std::pair<std::string, HashResult>>& results
) {
    out << "{\n";
    out << "  \"algorithm\": \"" << algo << "\",\n";
    out << "  \"hash_size\": " << size << ",\n";
    out << "  \"hash_bits\": " << (size * size) << ",\n";
    out << "  \"files\": {\n";
    for (size_t i = 0; i < results.size(); i++) {
        const auto& [path, hash] = results[i];
        out << "    \"" << path << "\": \"" << hash.to_hex() << "\"";
        if (i + 1 < results.size()) out << ",";
        out << "\n";
    }
    out << "  }\n";
    out << "}\n";
}


int main(int argc,char**argv){if(argc!=2)return 1;write_hash_json(std::cout,"dhash",2,{{argv[1],HashResult{{0},2}}});}
