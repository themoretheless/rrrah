import pathlib,hashlib,subprocess,tempfile
source=pathlib.Path(".dedup-work/cbird38-20261021/src/engine.cpp").read_bytes()
assert hashlib.sha1(b"blob "+str(len(source)).encode()+b"\0"+source).hexdigest()=="189a15e4c5e115a6646dee51fad79c3076ebafea"
text=source.decode();start=text.index("void Engine::commit() {");end=text.index("\nvoid Engine::update",start)
body=text[start:end]
program=r'''#include <vector>
#include <cstdio>
#include <cstdlib>
struct Batch: std::vector<int>{int count()const{return size();}};
struct Database{bool accept=true;std::vector<int> stored;int calls=0;void add(Batch& b){++calls;if(!accept)return;stored.insert(stored.end(),b.begin(),b.end());}};
struct Engine{Database* db;Batch _batch;void commit();};
BODY
int main(){Database db;Engine e{&db,{}};e._batch.push_back(42);e.commit();if(db.stored!=std::vector<int>{42}||!e._batch.empty())std::abort();std::puts("PASS successful store clears batch");
 db.stored.clear();db.calls=0;db.accept=false;e._batch.push_back(42);e.commit();if(!db.stored.empty()||!e._batch.empty()||db.calls!=1)std::abort();
 db.accept=true;e.commit();if(!db.stored.empty()||db.calls!=1)std::abort();std::puts("PASS reproduced silent add refusal losing pending batch and retry input");}
'''.replace("BODY",body)
with tempfile.TemporaryDirectory(prefix="cbird-batch-") as folder:
 root=pathlib.Path(folder);cpp=root/"control.cpp";cpp.write_text(program);binary=root/"control"
 subprocess.run(["clang++","-std=c++17","-O2","-Wall","-Wextra",str(cpp),"-o",str(binary)],check=True)
 subprocess.run([str(binary)],check=True)
