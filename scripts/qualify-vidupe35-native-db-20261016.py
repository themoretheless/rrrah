"""Compile unchanged upstream Db against real Qt/SQLite and a metadata-only Video adapter."""
from pathlib import Path
import hashlib,json,os,shutil,subprocess
R=Path(__file__).resolve().parents[1];D=R/'docs/research/dedup-500';W=R/'.dedup-work/vidupe35-20261016';S=W/'source';B=W/'native-db-v2';B.mkdir(exist_ok=False);h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
for name in ['db.cpp','db.h']:shutil.copyfile(S/name,B/name);assert h(S/name)==h(B/name)
(B/'video.h').write_text('''#pragma once
#include <QFileInfo>
#include <QDir>
#include <QFile>
#include <QDateTime>
class Video { public: QDateTime modified; int64_t size=0,duration=0; int bitrate=0; double framerate=0; QString codec,audio; short width=0,height=0; };
''')
source=(S/'mainwindow.cpp').read_bytes().decode();begin=source.index('void MainWindow::deleteTemporaryFiles() const');end=source.index('\nvoid MainWindow::dropEvent',begin);cleanup=source[begin:end]
main=r'''#include <QCoreApplication>
#include <QDirIterator>
#include <QSqlDatabase>
#include <iostream>
#include "video.h"
#include "db.h"
class MainWindow { public: void deleteTemporaryFiles() const; };
CLEANUP
void check(bool b,const char *name){if(!b){std::cerr<<"FAIL "<<name<<"\n";std::exit(2);}std::cout<<"PASS "<<name<<"\n";}
void file(const QString &name,const QByteArray &data,const QDateTime &time){QDir().mkpath(QFileInfo(name).absolutePath());QFile f(name);if(!f.open(QIODevice::WriteOnly)||f.write(data)!=data.size()||!f.flush()||!f.setFileTime(time,QFileDevice::FileModificationTime))std::exit(3);}
int main(int argc,char **argv){QCoreApplication app(argc,argv);check(argc==2,"owned_argument");QString root=QString::fromLocal8Bit(argv[1]);check(QDir::tempPath()==root+"/tmp","temp_path_owned_guard");check(QSqlDatabase::drivers().contains("QSQLITE"),"real_sqlite_driver");auto t=QDateTime::fromMSecsSinceEpoch(1700000000123,Qt::UTC);QString a=root+"/a/same.mp4",b=root+"/b/same.mp4";file(a,"AAAA",t);file(b,"BBBBBBBB",t);Video original;original.size=4;original.duration=12000;original.width=4;original.height=3;original.codec="owned";
{Db da(a),db(b);check(da.uniqueId()==db.uniqueId(),"same_basename_mtime_key_collision");da.writeMetadata(original);Video observed;check(db.readMetadata(observed)&&observed.size==4&&QFileInfo(b).size()==8,"different_path_reads_wrong_metadata");QByteArray pixels("x\0y",3);da.writeCapture(8,pixels);check(db.readCapture(8)==pixels,"different_path_reads_wrong_capture");}
{Db before(a);auto id=before.uniqueId();before.writeMetadata(original);file(a,"ZZZZ",t);Db after(a);Video observed;check(id==after.uniqueId()&&after.readMetadata(observed)&&observed.codec=="owned","same_size_restored_mtime_stale_metadata");}
QString u=root+QString::fromUtf8("/unicode/名字.mp4"),v=root+QString::fromUtf8("/unicode/文字.mp4");file(u,"a",t);file(v,"b",t);{Db x(u),y(v);check(x.uniqueId()==y.uniqueId(),"latin1_unicode_name_collision");}
QString newer=root+"/newer/same.mp4";file(newer,"a",t.addMSecs(1));{Db x(a),y(newer);check(x.uniqueId()!=y.uniqueId(),"different_millisecond_separates_keys");}
QString quote=root+"/quote.mp4";file(quote,"a",t);{Db x(quote);Video bad=original;bad.codec="a'b";x.writeMetadata(bad);Video observed;check(!x.readMetadata(observed),"unescaped_quote_metadata_write_lost");}
for(auto name:{"Vidupe-owned","W-unrelated","A-unrelated"})QDir().mkpath(root+"/tmp/"+name);
MainWindow window;window.deleteTemporaryFiles();bool unrelatedDeleted=!QFileInfo::exists(root+"/tmp/W-unrelated");check(unrelatedDeleted,"cleanup_removes_owned_unrelated_prefix_W");check(QFileInfo::exists(root+"/tmp/A-unrelated"),"cleanup_keeps_owned_prefix_A");std::cout<<"QT_VERSION="<<qVersion()<<"\n";return 0;}
'''.replace('CLEANUP',cleanup)
(B/'main.cpp').write_text(main);flags=subprocess.check_output(['pkg-config','--cflags','--libs','Qt6Widgets','Qt6Sql'],text=True).split();exe=B/'controls';cmd=['c++','-std=c++17','-O2','-DAPP_VERSION="qualification"',str(B/'db.cpp'),str(B/'main.cpp'),'-o',str(exe),*flags]
with (B/'build.log').open('x') as stream:result=subprocess.run(cmd,stdout=stream,stderr=subprocess.STDOUT)
assert result.returncode==0,'Native compilation failed, rawlog retained';logs=[]
for repeat in [1,2]:
 case=B/f'run{repeat}';case.mkdir();tmp=case/'tmp';tmp.mkdir();runexe=case/'controls';shutil.copyfile(exe,runexe);runexe.chmod(0o755);log=case/'run.log';env=dict(os.environ,TMPDIR=str(tmp))
 with log.open('x') as stream:result=subprocess.run([str(runexe),str(case)],env=env,stdout=stream,stderr=subprocess.STDOUT,timeout=30)
 assert result.returncode==0,log.read_text();text=log.read_text();assert text.count('PASS ')==12;logs.append(log)
out=D/'vidupe35-native-db-controls-20261016.json';assert not out.exists();out.write_text(json.dumps({'status':'verified_selected_original_db_and_cleanup_native_controls','whole_repo_complete':False,'producer_sha256':h(__file__),'runs':2,'controls_per_run':12,'qt_version':subprocess.check_output(['qmake','-query','QT_VERSION'],text=True).strip(),'source_pins':{str(x.relative_to(R)):h(x) for x in [S/'db.cpp',S/'db.h',S/'video.h',S/'mainwindow.cpp',B/'video.h',B/'main.cpp',B/'db.cpp',B/'db.h',exe,B/'build.log',*logs]},'command':cmd,'scope':'Entire unchanged Db.cpp/Db.h, actual Qt6.12/SQLite. Video adapter contains only original metadata fields, no OpenCV/video/QRunnable signals. Complete unchanged deleteTemporaryFiles body extracted to minimal class, TMPDIR exact-owned path asserted before operation; only owned synthetic directories/files touched.12controls twice including guards/native-driver check, not full application/testsuite/Qt5/runtime/performance proof.','findings':['Basename/mtime key aliases different paths and returns foreign metadata/capture','Unrepresentable Latin1 Unicode filenames alias; same-size restoredmtime changes are stale','SQL metadata interpolation loses codec containing single quote','Original cleanup compare==1 removes unrelated W-prefix directory inside owned fixture; A-prefix survives, actualQt6 only'],'limits':['No user videos/temp directories touched, no FFmpeg/video decode/SSIM/native app qualification','No performance/production adoption claim; Windows Qt5 compatibility not tested']},indent=2)+'\n');print('12actualQt/SQLite controls twice passed on owned fixtures')
