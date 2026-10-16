#include "ETC_Decoder.hpp"
#include <cstdint>
extern "C" bool probe(const unsigned char* source, short* output, int type) {
 return ETC_Decoder::Decode(source, reinterpret_cast<unsigned char*>(output),4,4,8,2,static_cast<ETC_Decoder::InputType>(type));
}
