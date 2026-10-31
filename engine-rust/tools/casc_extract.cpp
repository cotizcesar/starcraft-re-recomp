#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <string>
#include <vector>
#include "CascLib/src/CascLib.h"

static bool read_file(HANDLE hStorage, const char *name, const std::string &outDir) {
    HANDLE hFile = NULL;
    if (!CascOpenFile(hStorage, name, 0, CASC_OPEN_BY_NAME, &hFile)) {
        fprintf(stderr, "open failed: %s\n", name);
        return false;
    }
    ULONGLONG size = 0;
    CascGetFileSize64(hFile, &size);
    std::vector<unsigned char> buf((size_t)size);
    DWORD total = 0;
    while (total < size) {
        DWORD got = 0;
        DWORD want = (DWORD)((size - total) > 0x10000000 ? 0x10000000 : (size - total));
        if (!CascReadFile(hFile, buf.data() + total, want, &got) || got == 0) break;
        total += got;
    }
    CascCloseFile(hFile);
    std::string out = outDir + "/" + name;
    for (auto &c : out) if (c == '\\') c = '/';
    // mkdir -p
    std::string dir;
    for (size_t i = 0; i < out.size(); i++) {
        if (out[i] == '/') {
            dir = out.substr(0, i);
            std::string cmd = "mkdir -p '" + dir + "'";
            (void)system(cmd.c_str());
        }
    }
    FILE *f = fopen(out.c_str(), "wb");
    if (!f) { fprintf(stderr, "write failed: %s\n", out.c_str()); return false; }
    fwrite(buf.data(), 1, total, f);
    fclose(f);
    printf("%s\t%llu\n", name, (unsigned long long)total);
    return true;
}

int main(int argc, char **argv) {
    if (argc < 3) {
        fprintf(stderr, "usage: %s <storage> list [mask] | extract <outdir> <mask> [listfile]\n", argv[0]);
        return 2;
    }
    const char *storage = argv[1];
    std::string cmd = argv[2];
    HANDLE hStorage = NULL;
    if (!CascOpenStorage(storage, 0, &hStorage)) {
        fprintf(stderr, "CascOpenStorage failed: %u\n", GetCascError());
        return 1;
    }
    int rc = 0;
    if (cmd == "list") {
        const char *mask = argc > 3 ? argv[3] : "*";
        CASC_FIND_DATA fd;
        HANDLE hFind = CascFindFirstFile(hStorage, mask, &fd, NULL);
        if (!hFind) {
            fprintf(stderr, "find failed: %u\n", GetCascError());
            rc = 1;
        } else {
            do {
                printf("%s\t%llu\n", fd.szFileName, (unsigned long long)fd.FileSize);
            } while (CascFindNextFile(hFind, &fd));
            CascFindClose(hFind);
        }
    } else if (cmd == "extract") {
        if (argc < 5) { fprintf(stderr, "extract needs outdir and mask\n"); rc = 2; }
        else {
            const char *outDir = argv[3];
            const char *mask = argv[4];
            const char *listfile = argc > 5 ? argv[5] : NULL;
            CASC_FIND_DATA fd;
            HANDLE hFind = CascFindFirstFile(hStorage, mask, &fd, listfile);
            if (!hFind) {
                fprintf(stderr, "find failed: %u\n", GetCascError());
                rc = 1;
            } else {
                do {
                    if (!read_file(hStorage, fd.szFileName, outDir)) rc = 1;
                } while (CascFindNextFile(hFind, &fd));
                CascFindClose(hFind);
            }
        }
    } else {
        fprintf(stderr, "unknown command\n");
        rc = 2;
    }
    CascCloseStorage(hStorage);
    return rc;
}
