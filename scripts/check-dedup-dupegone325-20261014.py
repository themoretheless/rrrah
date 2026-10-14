#!/usr/bin/env python3
from pathlib import Path
import hashlib
import json
import os
import subprocess
import tempfile

R=Path(__file__).resolve().parents[1];D=R/'docs/research';S=Path('/private/tmp/rrrah-dupegone325-source-20261014');F=Path('/private/tmp/rrrah-dupegone325-controls-20261014')
O=D/'dedup-dupegone325-controls-20261014.json';assert not O.exists()
original=json.loads((F/'original-command.json').read_text());assert "library 'boost_system' not found" in (F/'original-build.log').read_text()
command=[arg for arg in original if arg!='-lboost_system'];command[command.index('-o')+1]=str(F/'adapted-cli')
build=subprocess.run(command,capture_output=True,text=True);(F/'adapted-build.log').write_text(build.stdout+build.stderr);assert build.returncode==0,build.stderr
driver=r'''
#include <cassert>
#include <set>
#include <iomanip>
#include <sstream>
#define main upstream_main
#include "/private/tmp/rrrah-dupegone325-source-20261014/dupegone.cpp"
#undef main
void put(const string& path,const string& bytes){ofstream stream(path,ios::binary);stream.write(bytes.data(),bytes.size());}
string hex(digest value){ostringstream s;for(auto byte:value)s<<std::hex<<setfill('0')<<setw(2)<<int(byte);return s.str();}
int main(int argc,char** argv){assert(argc==2);string root=argv[1];
 digest a{},b{};a[0]=1;a[1]=2;b[0]=3;assert(a!=b && hash<digest>{}(a)==hash<digest>{}(b));cout<<"bucket_collision\n";
 unordered_multimap<digest,int> table;table.insert({a,1});table.insert({b,2});assert(table.count(a)==1&&table.count(b)==1);cout<<"bucket_collision_preserves_digest_keys\n";
 set<size_t> hashes;for(unsigned i=0;i<65536;i++){digest key{};key[0]=i&255;key[1]=i>>8;hashes.insert(hash<digest>{}(key));}assert(hashes.size()==256);cout<<"hash_table_hash_has_256_values_for_65536_keys\n";
 string abc=root+"/abc",other=root+"/other",empty=root+"/empty";put(abc,"abc");put(other,"adc");put(empty,"");assert(hex(sha1(abc,3))=="a9993e364706816aba3e25717850c26c9cd0d89d");cout<<"real_sha1_known_vector\n";
 assert(sha1(root+"/absent",17)==sha1(empty,17));cout<<"missing_file_hashes_like_empty\n";
 assert(sha1(abc,1)!=sha1(other,1));cout<<"one_byte_limit_reads_beyond_prefix\n";
 file_info cached(abc,3);auto before=cached.fullhash();put(abc,"xyz");assert(cached.fullhash()==before&&cached.fullhash()!=sha1(abc,3));cout<<"cached_hash_survives_content_mutation\n";
 string large=root+"/large";string bytes(65537,'A');put(large,bytes);file_info stored(large,bytes.size());stored.firsthash();auto first=stored;auto initial=first.fullhash();bytes.back()='B';put(large,bytes);auto second=stored;assert(second.fullhash()!=initial && first.fullhash()==initial);cout<<"copied_fullhash_cache_not_retained_in_stored_record\n";
 multimap<int,file_info> sizes;sizes.insert(make_pair(uint64_t(1),file_info("a",1)));sizes.insert(make_pair((uint64_t(1)<<32)+1,file_info("b",(uint64_t(1)<<32)+1)));assert(sizes.count(1)==2);cout<<"large_sizes_narrow_into_same_32bit_key\n";
 string left=root+"/left",right=root+"/right";bytes.back()='A';put(left,bytes);bytes.back()='B';put(right,bytes);file_info x(left,65537),y(right,65537);assert(x.firsthash()==y.firsthash()&&x.fullhash()!=y.fullhash());cout<<"full_hash_distinguishes_same_prefix_different_tail\n";
}
'''
source=F/'original-functions-controls.cpp';source.write_text(driver)
cmd=list(command);cmd[cmd.index(str(S/'dupegone.cpp'))]=str(source);cmd[cmd.index('-o')+1]=str(F/'function-controls')
build=subprocess.run(cmd,capture_output=True,text=True);(F/'function-build.log').write_text(build.stdout+build.stderr);assert build.returncode==0,build.stderr
checks=[]
with tempfile.TemporaryDirectory(prefix='rrrah-dupegone325-',dir='/private/tmp') as temporary:
    root=Path(temporary);functions=root/'functions';functions.mkdir();run=subprocess.run([str(F/'function-controls'),str(functions)],capture_output=True,text=True);(F/'functions.log').write_text(run.stdout+run.stderr);assert run.returncode==0,run.stderr
    checks.extend(run.stdout.splitlines());assert len(checks)==10
    cli=F/'adapted-cli'
    def invoke(name,directory):
        result=subprocess.run([str(cli),str(directory),'1'],capture_output=True,text=True)
        (F/(name+'.log')).write_text(result.stdout+result.stderr);assert result.returncode==0,result.stderr;return result
    noargs=subprocess.run([str(cli)],capture_output=True,text=True);assert noargs.returncode==0 and 'Usage:' in noargs.stdout;checks.append('missing_arguments_return_success')
    copies=root/'copies';copies.mkdir()
    for i in range(4):(copies/str(i)).write_bytes(b'abc')
    result=invoke('copies',copies);assert 'found 6 duplicates' in result.stderr;checks.append('four_equal_copies_emit_six_pairs')
    different=root/'different';different.mkdir();(different/'a').write_bytes(b'AA');(different/'b').write_bytes(b'BB')
    result=invoke('different',different);assert 'found 0 duplicates' in result.stderr;checks.append('different_same_size_files_not_reported')
    aliases=root/'aliases';aliases.mkdir();(aliases/'a').write_bytes(b'abc');os.link(aliases/'a',aliases/'b')
    result=invoke('aliases',aliases);assert 'found 1 duplicates' in result.stderr;checks.append('hardlink_aliases_reported_as_duplicate_path_pair')
    unreadable=root/'unreadable';unreadable.mkdir();paths=[unreadable/'a',unreadable/'b'];paths[0].write_bytes(b'AA');paths[1].write_bytes(b'BB')
    try:
        for path in paths:path.chmod(0)
        assert not any(os.access(path,os.R_OK) for path in paths),'Control requires ordinary non-root file permissions'
        result=invoke('unreadable',unreadable);assert 'found 1 duplicates' in result.stderr;checks.append('distinct_unreadable_files_reported_as_duplicate')
    finally:
        for path in paths:path.chmod(0o600)
assert len(checks)==15
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
O.write_text(json.dumps(dict(status='fifteen_original_cpp_function_and_cli_controls_confirmed',repository='phiresky/dupegone',commit='8044570b22cc0e87b6b4302611a41e0a71bd3520',checks=checks,count=len(checks),build_adapter='Original C++11 source unchanged; remove obsolete -lboost_system flag because installed Boost.System is header-only. Initial documented-library link failure retained. Actual installed Boost.Filesystem and OpenSSL used.',pins={str(p):sha(p) for p in [*S.iterdir(),source,F/'adapted-cli',F/'function-controls',F/'original-build.log',F/'adapted-build.log',F/'function-build.log',F/'functions.log',*[F/(name+'.log') for name in ['copies','different','aliases','unreadable']],Path('/opt/homebrew/lib/libboost_filesystem.dylib'),Path('/opt/homebrew/opt/openssl@3/lib/libcrypto.dylib')] if p.is_file()},scope='Actual original CLI on synthetic local files only; unchanged original functions included under renamed main for ten direct controls. No actual SHA1 collision, multi-GB sparse-file CLI, performance benchmark, source concurrency stress or cross-platform qualification. A digest bucket collision only impacts performance: key equality remains full digest. No production adoption.'),indent=2)+'\n');print(f'{len(checks)} controls confirmed')
