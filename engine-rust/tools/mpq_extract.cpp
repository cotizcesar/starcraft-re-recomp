// Minimal StormLib wrapper used to extract `staredit\scenario.chk` from a
// retail .scm/.scx MPQ archive.
//
// Build (from the GalaxyCraft repository root):
//   g++ -O2 -std=c++17 -I StormLib/src tools/mpq_extract.cpp -o mpq_tool \
//       -L StormLib/build -lstorm -lz -lbz2
//
// Usage:
//   ./mpq_tool <map.scm> list
//   ./mpq_tool <map.scm> extract <out.chk> 'staredit\scenario.chk'
#include <StormLib.h>
#include <cstdio>
#include <cstring>
#include <string>
#include <vector>

static int extract(HANDLE archive, const char *name, const char *out_path) {
    HANDLE file = nullptr;
    if (!SFileOpenFileEx(archive, name, SFILE_OPEN_FROM_MPQ, &file)) {
        std::fprintf(stderr, "open failed: %s (%u)\n", name, SErrGetLastError());
        return 1;
    }
    DWORD size = SFileGetFileSize(file, nullptr);
    std::vector<unsigned char> data(size);
    DWORD read = 0;
    SFileReadFile(file, data.data(), size, &read, nullptr);
    SFileCloseFile(file);
    FILE *out = std::fopen(out_path, "wb");
    if (!out) {
        std::perror(out_path);
        return 1;
    }
    std::fwrite(data.data(), 1, read, out);
    std::fclose(out);
    std::printf("%s -> %s (%u bytes)\n", name, out_path, read);
    return 0;
}

int main(int argc, char **argv) {
    if (argc < 3) {
        std::fprintf(stderr,
                     "usage: %s <map> list | %s <map> extract <out.chk> <member>\n",
                     argv[0], argv[0]);
        return 2;
    }
    HANDLE archive = nullptr;
    if (!SFileOpenArchive(argv[1], 0, MPQ_OPEN_READ_ONLY, &archive)) {
        std::fprintf(stderr, "archive failed: %u\n", SErrGetLastError());
        return 1;
    }
    int rc = 0;
    if (std::strcmp(argv[2], "list") == 0) {
        SFILE_FIND_DATA fd;
        HANDLE find = SFileFindFirstFile(archive, "*", &fd, nullptr);
        if (!find) {
            std::fprintf(stderr, "find failed: %u\n", SErrGetLastError());
            rc = 1;
        } else {
            do {
                std::printf("%s\t%u\n", fd.cFileName, fd.dwFileSize);
            } while (SFileFindNextFile(find, &fd));
            SFileFindClose(find);
        }
    } else if (std::strcmp(argv[2], "extract") == 0 && argc >= 5) {
        rc = extract(archive, argv[4], argv[3]);
    } else {
        std::fprintf(stderr, "unknown command\n");
        rc = 2;
    }
    SFileCloseArchive(archive);
    return rc;
}
