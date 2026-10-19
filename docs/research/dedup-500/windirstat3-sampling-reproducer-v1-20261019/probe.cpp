#include <cstdint>
extern "C" uint64_t sampled_offset(uint64_t size,uint64_t total) {
const uint64_t Mi=1024*1024; const auto span=size-Mi; const auto block=total/Mi; const auto withinBlock=total%Mi;
return span / 3 * block + span % 3 * block / 3 + withinBlock;
}
