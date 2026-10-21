from pathlib import Path
import tempfile,subprocess,sys,os
root=Path(sys.argv[1]).resolve()
code=r'''
#include <cassert>
#include <cstdio>
#include <string>
#include <functional>
using uint=unsigned;using QString=std::string;
#define Q_ALWAYS_INLINE inline
#define qUtf8Printable(s) ((s).c_str())
inline uint qHash(unsigned long long x){return std::hash<unsigned long long>{}(x);}
#include "fsutil.h"
int main(int argc,char**argv){assert(argc==5);FileId a{QString(argv[1])},b{QString(argv[2])},c{QString(argv[3])},d{QString(argv[4])};assert(a.isValid()&&b.isValid()&&a==b);puts("{\"name\":\"hardlink-shares-device-inode\",\"passed\":true}");assert(c.isValid()&&a==c);puts("{\"name\":\"stat-follows-symlink-to-same-file-id\",\"passed\":true}");assert(!d.isValid());puts("{\"name\":\"missing-path-is-invalid\",\"passed\":true}");}
'''
cpp=root/'fileid-controls.cpp';binary=root/'fileid-controls';cpp.write_text(code)
subprocess.run(['clang++','-std=c++17','-O2','-I',str(root/'src'),str(cpp),'-o',str(binary)],check=True)
with tempfile.TemporaryDirectory(prefix='cbird38-fileid-') as td:
 p=Path(td);(p/'original').write_bytes(b'synthetic file identity only');os.link(p/'original',p/'hard');os.symlink('original',p/'symbolic');subprocess.run([str(binary),str(p/'original'),str(p/'hard'),str(p/'symbolic'),str(p/'missing')],check=True)
