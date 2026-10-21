import pathlib, hashlib, subprocess, tempfile
source=pathlib.Path(".dedup-work/cbird38-20261021/src/commands.cpp").read_bytes()
assert hashlib.sha1(b"blob "+str(len(source)).encode()+b"\0"+source).hexdigest()=="13ac2a0573b0cb35d129a0b2923e58f4b56b4c70"
text=source.decode()
start=text.index("[booleanExp](const QVariant& lhs, const QVariant& rhs)")
end=text.index("\n    };",start)+len("\n    }")
body=text[start:end]
program=r'''#include <vector>
#include <utility>
#include <cstdlib>
#include <cstdio>
using QVariant=bool;
struct Expression { bool value; bool rhsIsNeedle()const{return false;} bool eval(bool)const{return value;} bool eval(bool,bool)const{return value;} };
enum {BooleanEnd=-1,BooleanAnd=0,BooleanOr=1};
bool original(bool a,bool b,bool c,int first,int second){
 std::vector<std::pair<Expression,int>> booleanExp={{{a},first},{{b},second},{{c},BooleanEnd}};
 auto evaluate=BODY;
 return evaluate(false,false);
}
int main(){
 for(int mask=0;mask<8;++mask){bool a=mask&1,b=mask&2,c=mask&4;
 if(original(a,b,c,BooleanAnd,BooleanAnd)!=(a&&b&&c))std::abort();
 if(original(a,b,c,BooleanOr,BooleanOr)!=(a||b||c))std::abort();}
 std::puts("PASS homogeneous AND/OR: 16 truth-table cases");
 int differences=0;
 for(int mask=0;mask<8;++mask){bool a=mask&1,b=mask&2,c=mask&4;bool got=original(a,b,c,BooleanAnd,BooleanOr),expected=(a&&b)||c;
 if(got!=expected){++differences;std::printf("COUNTEREXAMPLE %d && %d || %d: original=%d expected=%d\n",a,b,c,got,expected);}}
 if(differences!=2)std::abort();
 std::puts("PASS reproduced two mixed-operator disagreements out of eight cases");
}
'''.replace("BODY",body)
with tempfile.TemporaryDirectory(prefix="cbird-boolean-") as folder:
 root=pathlib.Path(folder); cpp=root/"control.cpp";cpp.write_text(program);binary=root/"control"
 subprocess.run(["clang++","-std=c++17","-O2","-Wall","-Wextra",str(cpp),"-o",str(binary)],check=True)
 subprocess.run([str(binary)],check=True)
