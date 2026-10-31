/* Minimal in-memory wrapper around StormLib's MIT-licensed PKWARE DCL
 * explode implementation.  Vendored so the reconstruction does not depend on
 * a system StormLib installation. */
#include <string.h>
#include <stdlib.h>
#include "pklib.h"

typedef struct {
    const unsigned char *in;
    unsigned int in_len;
    unsigned int in_pos;
    unsigned char *out;
    unsigned int out_cap;
    unsigned int out_pos;
} SC_BUF;

static unsigned int sc_read(char *buf, unsigned int *size, void *param) {
    SC_BUF *b = (SC_BUF *)param;
    unsigned int avail = b->in_len - b->in_pos;
    unsigned int n = *size < avail ? *size : avail;
    if (n) memcpy(buf, b->in + b->in_pos, n);
    b->in_pos += n;
    return n;
}

static void sc_write(char *buf, unsigned int *size, void *param) {
    SC_BUF *b = (SC_BUF *)param;
    unsigned int n = *size;
    if (b->out_pos + n > b->out_cap) n = b->out_cap - b->out_pos;
    if (n) memcpy(b->out + b->out_pos, buf, n);
    b->out_pos += n;
}

/* Returns 0 on success (StormLib's explode returns CMP_NO_ERROR == 0 when the
 * expander finishes) and writes the decompressed length to *out_len. */
int sc_pklib_explode(const unsigned char *in, unsigned int in_len,
                     unsigned char *out, unsigned int out_cap,
                     unsigned int *out_len) {
    SC_BUF b;
    TDcmpStruct *work = (TDcmpStruct *)malloc(sizeof(TDcmpStruct));
    unsigned int rc;
    if (!work) return -2;
    memset(&b, 0, sizeof(b));
    b.in = in; b.in_len = in_len; b.in_pos = 0;
    b.out = out; b.out_cap = out_cap; b.out_pos = 0;
    memset(work, 0, sizeof(TDcmpStruct));
    rc = explode(sc_read, sc_write, (char *)work, &b);
    free(work);
    *out_len = b.out_pos;
    return (int)rc;
}
