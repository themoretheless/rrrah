import pathlib,hashlib,tempfile,subprocess
header=pathlib.Path(".dedup-work/cbird38-20261021/src/stringutil.h").resolve();data=header.read_bytes()
assert hashlib.sha1(b"blob "+str(len(data)).encode()+b"\0"+data).hexdigest()=="fdf28e8429f05cc0318f9b01039a880eea584a7e"
program=r'''#include <algorithm>
#include <cstring>
#include <cstdio>
#include <cstdlib>
#include "HEADER"
int distance(const char* a,const char* b){return levenshteinDistance(a,std::strlen(a),b,std::strlen(b));}
void check(bool b){if(!b)std::abort();}
int main(){check(distance("kitten","sitting")==3 && distance("same","same")==0);std::puts("PASS standard substitution and equality controls");
 check(distance("abc","abcd")==0 && distance("abcd","abc")==0);std::puts("PASS reproduced prefix extension/deletion distance0 instead of1");
 check(distance("","abc")==0 && distance("abc","")==0);std::puts("PASS reproduced empty/nonempty distance0 instead of3");}
'''.replace("HEADER",str(header))
with tempfile.TemporaryDirectory(prefix="cbird-string-") as folder:
 root=pathlib.Path(folder);cpp=root/"control.cpp";cpp.write_text(program);binary=root/"control"
 subprocess.run(["clang++","-std=c++17","-O2","-Wall","-Wextra",str(cpp),"-o",str(binary)],check=True)
 subprocess.run([str(binary)],check=True)
