/* macOS test-only libc interposition; no production linkage, no root tracing. */
#include <sys/stat.h>
#include <stdatomic.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
static _Atomic uint64_t lstats, stats, fstatats, fstats;
static int fixture(const char *path) { return path && strstr(path,"/fixtures/"); }
static int counted_lstat(const char *path, struct stat *buf) { if(fixture(path)) atomic_fetch_add(&lstats,1); return lstat(path,buf); }
static int counted_stat(const char *path, struct stat *buf) { if(fixture(path)) atomic_fetch_add(&stats,1); return stat(path,buf); }
static int counted_fstatat(int fd,const char *path,struct stat *buf,int flags) { atomic_fetch_add(&fstatats,1); return fstatat(fd,path,buf,flags); }
static int counted_fstat(int fd,struct stat *buf) {atomic_fetch_add(&fstats,1); return fstat(fd,buf);}
#define INTERPOSE(replacement,original) __attribute__((used)) static struct {const void *replacement;const void *original;} interpose_##original __attribute__((section("__DATA,__interpose"))) = {(const void*)replacement,(const void*)original};
INTERPOSE(counted_lstat,lstat)
INTERPOSE(counted_stat,stat)
INTERPOSE(counted_fstatat,fstatat)
INTERPOSE(counted_fstat,fstat)
__attribute__((destructor)) static void report(void) {fprintf(stderr,"COVER_LIBC lstat=%llu stat=%llu fstatat_all=%llu fstat_all=%llu\n",(unsigned long long)atomic_load(&lstats),(unsigned long long)atomic_load(&stats),(unsigned long long)atomic_load(&fstatats),(unsigned long long)atomic_load(&fstats));}
