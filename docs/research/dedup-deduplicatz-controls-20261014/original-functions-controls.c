#include <pthread.h>
#include <unistd.h>
#include <sys/uio.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include <stdint.h>
#include <stdatomic.h>
#include <assert.h>
/* Controlled byte hash for worker framing tests only, not real xxHash. */
static uint64_t XXH3_64bits(const void* p, size_t n) {
    const unsigned char* bytes=p; uint64_t h=1469598103934665603ULL;
    for(size_t i=0;i<n;i++) h=(h^bytes[i])*1099511628211ULL;
    return h;
}
#define CHUNK_SIZE      65536
#define MAX_LINE_SIZE   16384
#define INIT_HT_SIZE    10000019UL
#define LOAD_FACTOR     0.85
#define PROBE_LIMIT     64
#define QUEUE_SIZE      256
#define IOV_BATCH       128
#define RING_DEPTH      32

static atomic_int   done_reading = 0;
static atomic_ulong total_seen   = 0;
static atomic_ulong total_unique = 0;
static int          out_fd;

// --- dynamic hashset ---
typedef struct {
    uint64_t*        table;
    size_t           size;
    atomic_ulong     count;
    pthread_rwlock_t lock;
} hashset_t;

static void hashset_init(hashset_t* hs, size_t sz) {
    hs->size = sz;
    hs->table = calloc(sz, sizeof(uint64_t));
    atomic_store(&hs->count, 0);
    pthread_rwlock_init(&hs->lock, NULL);
    if (!hs->table) {
        perror("calloc");
        exit(1);
    }
}

static void hashset_rehash(hashset_t* hs) {
    pthread_rwlock_wrlock(&hs->lock);
    size_t new_sz = hs->size * 2 + 1;
    while (1) {
        int p = 1;
        for (size_t d = 3; d * d <= new_sz; d += 2)
            if (new_sz % d == 0) {
                p = 0;
                break;
            }
        if (p) break;
        new_sz += 2;
    }
    uint64_t* new_tab = calloc(new_sz, sizeof(uint64_t));
    if (!new_tab) {
        perror("calloc rehash");
        exit(1);
    }
    for (size_t i = 0; i < hs->size; ++i) {
        uint64_t h = hs->table[i];
        if (!h) continue;
        size_t idx = h % new_sz;
        for (int j = 0; j < PROBE_LIMIT; ++j) {
            if (!new_tab[idx]) {
                new_tab[idx] = h;
                break;
            }
            idx = (idx + 1) % new_sz;
        }
    }
    free(hs->table);
    hs->table = new_tab;
    hs->size  = new_sz;
    pthread_rwlock_unlock(&hs->lock);
}

static int hashset_insert(hashset_t* hs, uint64_t h) {
    pthread_rwlock_rdlock(&hs->lock);
    size_t idx = h % hs->size;
    for (int j = 0; j < PROBE_LIMIT; ++j) {
        uint64_t exp = 0;
        if (__atomic_compare_exchange_n(&hs->table[idx], &exp, h, 0, __ATOMIC_SEQ_CST, __ATOMIC_SEQ_CST)) {
            size_t used = atomic_fetch_add(&hs->count, 1) + 1;
            pthread_rwlock_unlock(&hs->lock);
            if ((double)used / hs->size > LOAD_FACTOR)
                hashset_rehash(hs);
            return 1;
        }
        if (hs->table[idx] == h) {
            pthread_rwlock_unlock(&hs->lock);
            return 0;
        }
        idx = (idx + 1) % hs->size;
    }
    pthread_rwlock_unlock(&hs->lock);
    return 0;
}

static hashset_t hset;

// --- chunk queue ---
typedef struct { char* data; size_t sz; int last; } chunk_t;
typedef struct {
    chunk_t*        buf[QUEUE_SIZE];
    int             head, tail, count;
    pthread_mutex_t lock;
    pthread_cond_t  ne, nf;
} queue_t;

static queue_t queue;

static void queue_init(queue_t* q) {
    memset(q, 0, sizeof(*q));
    pthread_mutex_init(&q->lock, NULL);
    pthread_cond_init(&q->ne, NULL);
    pthread_cond_init(&q->nf, NULL);
}

static void queue_push(queue_t* q, chunk_t* c) {
    pthread_mutex_lock(&q->lock);
    while (q->count == QUEUE_SIZE)
        pthread_cond_wait(&q->nf, &q->lock);
    q->buf[q->tail] = c;
    q->tail = (q->tail + 1) % QUEUE_SIZE;
    q->count++;
    pthread_cond_signal(&q->ne);
    pthread_mutex_unlock(&q->lock);
}

static chunk_t* queue_pop(queue_t* q) {
    pthread_mutex_lock(&q->lock);
    while (q->count == 0) {
        if (atomic_load(&done_reading)) {
            pthread_mutex_unlock(&q->lock);
            return NULL;
        }
        pthread_cond_wait(&q->ne, &q->lock);
    }
    chunk_t* c = q->buf[q->head];
    q->head = (q->head + 1) % QUEUE_SIZE;
    q->count--;
    pthread_cond_signal(&q->nf);
    pthread_mutex_unlock(&q->lock);
    return c;
}

// --- worker ---
static void* worker_fn(void* _) {
    char* line = malloc(MAX_LINE_SIZE);
    struct iovec iovs[IOV_BATCH];
    int ic = 0;
    size_t llen = 0;
    while (1) {
        chunk_t* c = queue_pop(&queue);
        if (!c) break;
        for (size_t i = 0; i < c->sz; ++i) {
            char ch = c->data[i];
            if (llen < MAX_LINE_SIZE - 1)
                line[llen++] = ch;
            else {
                llen = 0;
                continue;
            }
            if (ch == '\n') {
                atomic_fetch_add(&total_seen, 1);
                uint64_t h = XXH3_64bits(line, llen);
                if (hashset_insert(&hset, h)) {
                    char* cp = malloc(llen);
                    memcpy(cp, line, llen);
                    iovs[ic].iov_base = cp;
                    iovs[ic].iov_len  = llen;
                    if (++ic == IOV_BATCH) {
                        writev(out_fd, iovs, ic);
                        for (int j = 0; j < ic; ++j)
                            free(iovs[j].iov_base);
                        ic = 0;
                    }
                    atomic_fetch_add(&total_unique, 1);
                }
                llen = 0;
            }
        }
        free(c);
    }
    if (ic > 0) {
        writev(out_fd, iovs, ic);
        for (int j = 0; j < ic; ++j)
            free(iovs[j].iov_base);
    }
    free(line);
    return NULL;
}


static void push(char* bytes,size_t length) {
    chunk_t* chunk=malloc(sizeof(*chunk));
    *chunk=(chunk_t){bytes,length,1}; queue_push(&queue,chunk);
}
int main(int argc,char** argv) {
    assert(argc==2);
    if(!strcmp(argv[1],"zero")) {
        hashset_init(&hset,101);
        assert(hashset_insert(&hset,0)==1);
        assert(hashset_insert(&hset,0)==1);
        assert(atomic_load(&hset.count)==2);
    } else if(!strcmp(argv[1],"probe")) {
        hashset_init(&hset,101);
        for(uint64_t i=0;i<64;i++) assert(hashset_insert(&hset,1+i*101)==1);
        assert(hashset_insert(&hset,1+64*101)==0);
        assert(atomic_load(&hset.count)==64);
    } else if(!strcmp(argv[1],"rehash")) {
        hashset_init(&hset,101);
        for(size_t i=0;i<65;i++) hset.table[i]=1+i*211;
        atomic_store(&hset.count,65); hashset_rehash(&hset);
        assert(hset.size==211);
        size_t occupied=0;for(size_t i=0;i<hset.size;i++) occupied+=hset.table[i]!=0;
        assert(occupied==64 && atomic_load(&hset.count)==65);
    } else {
        hashset_init(&hset,101);queue_init(&queue);
        FILE* output=tmpfile();assert(output);out_fd=fileno(output);
        char a[]="alpha\n",b[]="alpha\n";
        if(!strcmp(argv[1],"duplicate")){push(a,6);push(b,6);}
        else if(!strcmp(argv[1],"eof")){push(a,5);}
        else if(!strcmp(argv[1],"split")){push(a,2);push(a+2,4);}
        else if(!strcmp(argv[1],"borrow")){push(a,6);memcpy(a,"omega\n",6);}
        else if(!strcmp(argv[1],"long")) {
            char* longline=malloc(MAX_LINE_SIZE+2);memset(longline,'x',MAX_LINE_SIZE+1);longline[MAX_LINE_SIZE+1]='\n';push(longline,MAX_LINE_SIZE+2);
        } else return 2;
        atomic_store(&done_reading,1);worker_fn(NULL);
        fflush(output);fseek(output,0,SEEK_SET);char result[32]={0};size_t length=fread(result,1,sizeof(result),output);
        if(!strcmp(argv[1],"eof")) assert(length==0 && atomic_load(&total_seen)==0);
        else if(!strcmp(argv[1],"borrow")) assert(length==6 && !memcmp(result,"omega\n",6));
        else if(!strcmp(argv[1],"long")) assert(length==2 && !memcmp(result,"x\n",2));
        else assert(length==6 && !memcmp(result,"alpha\n",6));
        fclose(output);
    }
    puts("confirmed");return 0;
}
