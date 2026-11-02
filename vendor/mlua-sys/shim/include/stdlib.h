#ifndef MLUA_SHIM_STDLIB_H
#define MLUA_SHIM_STDLIB_H

#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

#define EXIT_SUCCESS 0
#define EXIT_FAILURE 1

typedef struct { int quot, rem; } div_t;
typedef struct { long quot, rem; } ldiv_t;
typedef struct { long long quot, rem; } lldiv_t;

void* malloc(size_t size);
void* calloc(size_t count, size_t size);
void* realloc(void* ptr, size_t size);
void free(void* ptr);
__attribute__((noreturn)) void abort(void);

double strtod(const char* s, char** end);
unsigned long strtoul(const char* s, char** end, int base);
unsigned long long strtoull(const char* s, char** end, int base);
int atoi(const char* s);

int abs(int x);
long labs(long x);
long long llabs(long long x);
div_t div(int a, int b);
ldiv_t ldiv(long a, long b);
lldiv_t lldiv(long long a, long long b);

#ifdef __cplusplus
}
#endif

#endif
