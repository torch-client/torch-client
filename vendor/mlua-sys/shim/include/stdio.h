#ifndef MLUA_SHIM_STDIO_H
#define MLUA_SHIM_STDIO_H

#include <stdarg.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

#define EOF (-1)

typedef struct mlua_shim_file FILE;

extern FILE* const stdout;
extern FILE* const stderr;
#define stdout (stdout)
#define stderr (stderr)

int snprintf(char* buf, size_t cap, const char* fmt, ...) __attribute__((format(printf, 3, 4)));
int vsnprintf(char* buf, size_t cap, const char* fmt, va_list args) __attribute__((format(printf, 3, 0)));
int printf(const char* fmt, ...) __attribute__((format(printf, 1, 2)));
int fprintf(FILE* f, const char* fmt, ...) __attribute__((format(printf, 2, 3)));
int vfprintf(FILE* f, const char* fmt, va_list args) __attribute__((format(printf, 2, 0)));
int fputc(int c, FILE* f);
int fputs(const char* s, FILE* f);
size_t fwrite(const void* data, size_t size, size_t count, FILE* f);
int fflush(FILE* f);
int puts(const char* s);

int remove(const char* path);

#ifdef __cplusplus
}
#endif

#endif
