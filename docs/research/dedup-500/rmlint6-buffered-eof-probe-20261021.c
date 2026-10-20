#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <errno.h>
#include <string.h>
typedef int gboolean; typedef size_t gsize; typedef uint64_t guint64;typedef void GThreadPool;typedef void RmDigest;
typedef struct {void *buf_sem;size_t buf_size;} RmHasher;
typedef struct {void *data;size_t len;void *digest;void *user_data;} RmBuffer;
#define TRUE 1
#define FALSE 0
#define G_MAXSIZE SIZE_MAX
#define MIN(a,b) ((a)<(b)?(a):(b))
#define g_strerror strerror
#define rm_log_info(...) ((void)0)
#define rm_log_perror(...) ((void)0)
#define rm_log_error_line(...) ((void)0)
#define rm_log_warning_line(...) ((void)0)
#define _(x) x
static size_t submitted;
static RmBuffer *rm_buffer_new(void *s,size_t n){(void)s;RmBuffer *b=calloc(1,sizeof(*b));b->data=malloc(n);return b;}
static void rm_buffer_free(void *s,RmBuffer *b){(void)s;free(b->data);free(b);}
static void rm_util_thread_pool_push(void *p,RmBuffer *b){(void)p;submitted+=b->len;rm_buffer_free(NULL,b);}
static void rm_hasher_request_readahead(int f,uint64_t a,uint64_t b){(void)f;(void)a;(void)b;}
static gboolean rm_hasher_buffered_read(RmHasher *hasher, GThreadPool *hashpipe,
                                        RmDigest *digest, char *path, guint64 start_offset,
                                        guint64 bytes_to_read, guint64 *bytes_actually_read) {
    FILE *fd = NULL;
    fd = fopen(path, "rb");
    if(fd == NULL) {
        rm_log_info("fopen(3) failed for %s: %s\n", path, g_strerror(errno));
        return FALSE;
    }

    gboolean read_to_eof = (bytes_to_read == 0);
    rm_hasher_request_readahead(fileno(fd), start_offset,
                                read_to_eof ? G_MAXSIZE : bytes_to_read);

    if(fseek(fd, start_offset, SEEK_SET) == -1) {
        rm_log_perror("fseek(3) failed");
        fclose(fd);
        return FALSE;
    }

    gboolean success = FALSE;
    gsize bytes_remaining = bytes_to_read;

    while(TRUE) {
        RmBuffer *buffer = rm_buffer_new(hasher->buf_sem, hasher->buf_size);
        gsize want_bytes = MIN(bytes_remaining, hasher->buf_size);
        gsize bytes_read = fread(buffer->data, 1, want_bytes, fd);

        if(ferror(fd) != 0) {
            rm_log_perror("fread(3) failed");
            rm_buffer_free(hasher->buf_sem, buffer);
            break;
        }

        bytes_remaining -= bytes_read;
        *bytes_actually_read += bytes_read;

        buffer->len = bytes_read;
        buffer->digest = digest;
        buffer->user_data = NULL;
        rm_util_thread_pool_push(hashpipe, buffer);

        if(read_to_eof && feof(fd)) {
            success = TRUE;
            break;
        } else if(bytes_remaining == 0) {
            success = TRUE;
            break;
        } else if(feof(fd)) {
            rm_log_error_line("Unexpected EOF in rm_hasher_buffered_read");
            break;
        } else if(bytes_read == 0) {
            rm_log_warning_line(_("Something went wrong reading %s; expected %lli bytes, "
                                "got %lli; ignoring"),
                              path, (long long)bytes_to_read,
                              (long long)*bytes_actually_read);
            break;
        }
    }
    fclose(fd);
    return success;
}

int main(int argc,char **argv){if(argc!=2)return 2;RmHasher h={NULL,4};uint64_t n=0;int ok=rm_hasher_buffered_read(&h,NULL,NULL,argv[1],0,0,&n);printf("zero_to_eof success=%d read=%llu submitted=%zu\n",ok,(unsigned long long)n,submitted);if(!ok||n!=0||submitted!=0)return 3;n=0;submitted=0;ok=rm_hasher_buffered_read(&h,NULL,NULL,argv[1],0,9,&n);printf("explicit_length success=%d read=%llu submitted=%zu\n",ok,(unsigned long long)n,submitted);return !(ok&&n==9&&submitted==9);}
