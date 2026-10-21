import hashlib, pathlib, subprocess, tempfile
header = pathlib.Path(".dedup-work/cbird38-20261021/src/hamm.h").resolve()
data = header.read_bytes()
assert hashlib.sha1(b"blob " + str(len(data)).encode() + b"\0" + data).hexdigest() == "25ab839fa04fd2926f494395f0bc2b9b1cd413ee"
with tempfile.TemporaryDirectory(prefix="cbird-hamming-") as folder:
    root = pathlib.Path(folder)
    source = root / "control.cpp"
    source.write_text(r'''#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include "HEADER"
int reference(uint64_t a, uint64_t b) { uint64_t x=a^b; int n=0; while(x) { n+=x&1; x>>=1; } return n; }
void check(bool b) { if(!b) std::abort(); }
int main() {
 check(hamm64(0,0)==0 && hamm64(0,UINT64_MAX)==64 && hamm64(UINT64_MAX,0)==64);
 std::puts("PASS endpoints 0 and 64, both directions");
 for(int i=0;i<64;++i) for(int j=0;j<64;++j) check(hamm64(uint64_t(1)<<i,uint64_t(1)<<j)==(i==j?0:2));
 std::puts("PASS 4096 single-bit pair distances");
 uint64_t state=0x72f13a9185ULL;
 auto next=[&]() { state^=state<<13;state^=state>>7;state^=state<<17;return state; };
 for(int i=0;i<10000;++i) { auto a=next(),b=next();check(hamm64(a,b)==reference(a,b));check(hamm64(a,b)==hamm64(b,a)); }
 std::puts("PASS 10000 deterministic pairs against bit-by-bit reference and symmetry");
}
'''.replace("HEADER", str(header)))
    binary=root/"control"
    subprocess.run(["clang++", "-std=c++17", "-O2", "-Wall", "-Wextra", str(source), "-o", str(binary)], check=True)
    subprocess.run([str(binary)], check=True)
