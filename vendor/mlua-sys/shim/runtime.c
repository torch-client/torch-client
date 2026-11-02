#include <ctype.h>
#include <errno.h>
#include <math.h>
#include <stdarg.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

__attribute__((noreturn)) void _emscripten_throw_longjmp(void);
double mlua_shim_parse_f64(const char* s, size_t len);
void mlua_shim_log(const char* s, size_t len);

/* The design is Emscripten's system/lib/compiler-rt/emscripten_setjmp.c
   (3.1.51, MIT), rewritten here. */

typedef struct
{
    uintptr_t id;
    uint32_t label;
} SetjmpEntry;

uintptr_t __THREW__ = 0;
int __threwValue = 0;

static uint32_t tempRet0;
static uintptr_t setjmpId;

void setTempRet0(uint32_t value)
{
    tempRet0 = value;
}

uint32_t getTempRet0(void)
{
    return tempRet0;
}

SetjmpEntry* saveSetjmp(uintptr_t* env, uint32_t label, SetjmpEntry* table, uint32_t size)
{
    setjmpId++;
    *env = setjmpId;

    for (;;)
    {
        for (uint32_t i = 0; i < size; i++)
        {
            if (table[i].id == 0)
            {
                table[i].id = setjmpId;
                table[i].label = label;
                table[i + 1].id = 0;
                setTempRet0(size);
                return table;
            }
        }

        size *= 2;
        table = (SetjmpEntry*)realloc(table, sizeof(SetjmpEntry) * (size + 1));
        if (!table)
            abort();
    }
}

uint32_t testSetjmp(uintptr_t id, SetjmpEntry* table, uint32_t size)
{
    for (uint32_t i = 0; i < size && table[i].id != 0; i++)
    {
        if (table[i].id == id)
            return table[i].label;
    }
    return 0;
}

void emscripten_longjmp(uintptr_t env, int value)
{
    if (value == 0)
        value = 1;

    if (__THREW__ == 0)
    {
        __THREW__ = env;
        __threwValue = value;
    }

    _emscripten_throw_longjmp();
}

__asm__(".globaltype __stack_pointer, i32\n");

uintptr_t mlua_shim_stack_save(void)
{
    uintptr_t sp;
    __asm__ volatile("global.get __stack_pointer\n\tlocal.set %0" : "=r"(sp));
    return sp;
}

void mlua_shim_stack_restore(uintptr_t sp)
{
    __asm__ volatile("local.get %0\n\tglobal.set __stack_pointer" : : "r"(sp));
}

static int errnoValue;

int* mlua_shim_errno(void)
{
    return &errnoValue;
}

void* memchr(const void* s, int c, size_t n)
{
    const unsigned char* p = (const unsigned char*)s;
    for (; n; n--, p++)
    {
        if (*p == (unsigned char)c)
            return (void*)p;
    }
    return NULL;
}

char* strcpy(char* dst, const char* src)
{
    char* d = dst;
    while ((*d++ = *src++))
        ;
    return dst;
}

char* strncpy(char* dst, const char* src, size_t n)
{
    size_t i = 0;
    for (; i < n && src[i]; i++)
        dst[i] = src[i];
    for (; i < n; i++)
        dst[i] = 0;
    return dst;
}

char* strcat(char* dst, const char* src)
{
    strcpy(dst + strlen(dst), src);
    return dst;
}

char* strncat(char* dst, const char* src, size_t n)
{
    char* d = dst + strlen(dst);
    for (; n && *src; n--)
        *d++ = *src++;
    *d = 0;
    return dst;
}

int strcmp(const char* a, const char* b)
{
    for (; *a && *a == *b; a++, b++)
        ;
    return (unsigned char)*a - (unsigned char)*b;
}

int strncmp(const char* a, const char* b, size_t n)
{
    for (; n; n--, a++, b++)
    {
        if (*a != *b || !*a)
            return (unsigned char)*a - (unsigned char)*b;
    }
    return 0;
}

char* strchr(const char* s, int c)
{
    for (;; s++)
    {
        if (*s == (char)c)
            return (char*)s;
        if (!*s)
            return NULL;
    }
}

char* strrchr(const char* s, int c)
{
    const char* last = NULL;
    for (;; s++)
    {
        if (*s == (char)c)
            last = s;
        if (!*s)
            return (char*)last;
    }
}

char* strstr(const char* haystack, const char* needle)
{
    size_t n = strlen(needle);
    if (n == 0)
        return (char*)haystack;
    for (; *haystack; haystack++)
    {
        if (*haystack == *needle && strncmp(haystack, needle, n) == 0)
            return (char*)haystack;
    }
    return NULL;
}

size_t strspn(const char* s, const char* accept)
{
    size_t n = 0;
    while (s[n] && strchr(accept, s[n]))
        n++;
    return n;
}

size_t strcspn(const char* s, const char* reject)
{
    size_t n = 0;
    while (s[n] && !strchr(reject, s[n]))
        n++;
    return n;
}

char* strpbrk(const char* s, const char* accept)
{
    s += strcspn(s, accept);
    return *s ? (char*)s : NULL;
}

enum
{
    CT_UPPER = 1,
    CT_LOWER = 2,
    CT_DIGIT = 4,
    CT_SPACE = 8,
    CT_PUNCT = 16,
    CT_CNTRL = 32,
    CT_HEX = 64,
    CT_BLANK = 128
};

#define U (CT_UPPER)
#define UX (CT_UPPER | CT_HEX)
#define L (CT_LOWER)
#define LX (CT_LOWER | CT_HEX)
#define D (CT_DIGIT)
#define P (CT_PUNCT)
#define C (CT_CNTRL)
#define S (CT_CNTRL | CT_SPACE)
#define T (CT_CNTRL | CT_SPACE | CT_BLANK)
#define B (CT_SPACE | CT_BLANK)

static const unsigned char ctypeTable[256] = {
    C, C, C, C, C, C, C, C, C, T, S, S, S, S, C, C,
    C, C, C, C, C, C, C, C, C, C, C, C, C, C, C, C,
    B, P, P, P, P, P, P, P, P, P, P, P, P, P, P, P,
    D, D, D, D, D, D, D, D, D, D, P, P, P, P, P, P,
    P, UX, UX, UX, UX, UX, UX, U, U, U, U, U, U, U, U, U,
    U, U, U, U, U, U, U, U, U, U, U, P, P, P, P, P,
    P, LX, LX, LX, LX, LX, LX, L, L, L, L, L, L, L, L, L,
    L, L, L, L, L, L, L, L, L, L, L, P, P, P, P, C,
};

#undef U
#undef UX
#undef L
#undef LX
#undef D
#undef P
#undef C
#undef S
#undef T
#undef B

static int ctypeOf(int c)
{
    return (unsigned)c < 256 ? ctypeTable[c] : 0;
}

int isalnum(int c) { return ctypeOf(c) & (CT_UPPER | CT_LOWER | CT_DIGIT); }
int isalpha(int c) { return ctypeOf(c) & (CT_UPPER | CT_LOWER); }
int isblank(int c) { return ctypeOf(c) & CT_BLANK; }
int iscntrl(int c) { return ctypeOf(c) & CT_CNTRL; }
int isdigit(int c) { return ctypeOf(c) & CT_DIGIT; }
int isgraph(int c) { return ctypeOf(c) & (CT_UPPER | CT_LOWER | CT_DIGIT | CT_PUNCT); }
int islower(int c) { return ctypeOf(c) & CT_LOWER; }
int isprint(int c) { return c == ' ' || isgraph(c); }
int ispunct(int c) { return ctypeOf(c) & CT_PUNCT; }
int isspace(int c) { return ctypeOf(c) & CT_SPACE; }
int isupper(int c) { return ctypeOf(c) & CT_UPPER; }
int isxdigit(int c) { return ctypeOf(c) & (CT_DIGIT | CT_HEX); }
int tolower(int c) { return isupper(c) ? c + ('a' - 'A') : c; }
int toupper(int c) { return islower(c) ? c - ('a' - 'A') : c; }

double frexp(double x, int* exp)
{
    union
    {
        double d;
        uint64_t u;
    } v = {x};
    int e = (int)(v.u >> 52 & 0x7ff);

    if (e == 0)
    {
        if (x == 0)
        {
            *exp = 0;
            return x;
        }
        x = frexp(x * 0x1p64, exp);
        *exp -= 64;
        return x;
    }
    if (e == 0x7ff)
    {
        *exp = 0;
        return x;
    }

    *exp = e - 0x3fe;
    v.u &= 0x800fffffffffffffull;
    v.u |= 0x3fe0000000000000ull;
    return v.d;
}

double modf(double x, double* ip)
{
    if (isinf(x))
    {
        *ip = x;
        return copysign(0.0, x);
    }
    double i = trunc(x);
    *ip = i;
    return copysign(x - i, x);
}

static int hexDigit(int c)
{
    if (c >= '0' && c <= '9')
        return c - '0';
    if (c >= 'a' && c <= 'f')
        return c - 'a' + 10;
    if (c >= 'A' && c <= 'F')
        return c - 'A' + 10;
    return -1;
}

static int matchWord(const char* s, const char* lowerWord)
{
    size_t i = 0;
    for (; lowerWord[i]; i++)
    {
        if (tolower((unsigned char)s[i]) != lowerWord[i])
            return 0;
    }
    return (int)i;
}

static const char* scanHexFloat(const char* s, double* out)
{
    uint64_t mantissa = 0;
    int sticky = 0;
    long exponent = 0;
    int digits = 0;
    int afterPoint = 0;

    for (;; s++)
    {
        if (*s == '.' && !afterPoint)
        {
            afterPoint = 1;
            continue;
        }
        int d = hexDigit((unsigned char)*s);
        if (d < 0)
            break;
        digits++;
        if (mantissa < (1ull << 60))
        {
            mantissa = mantissa * 16 + (uint64_t)d;
            if (afterPoint)
                exponent -= 4;
        }
        else
        {
            sticky |= d != 0;
            if (!afterPoint)
                exponent += 4;
        }
    }
    if (digits == 0)
        return NULL;

    if (*s == 'p' || *s == 'P')
    {
        const char* e = s + 1;
        int negative = 0;
        if (*e == '+' || *e == '-')
            negative = *e++ == '-';
        if (isdigit((unsigned char)*e))
        {
            long value = 0;
            for (; isdigit((unsigned char)*e); e++)
            {
                if (value < 100000)
                    value = value * 10 + (*e - '0');
            }
            exponent += negative ? -value : value;
            s = e;
        }
    }

    if (sticky)
        mantissa |= 1;
    if (exponent > 100000)
        exponent = 100000;
    if (exponent < -100000)
        exponent = -100000;
    *out = ldexp((double)mantissa, (int)exponent);
    return s;
}

double strtod(const char* s, char** end)
{
    const char* p = s;
    while (isspace((unsigned char)*p))
        p++;

    const char* numberStart = p;
    int negative = 0;
    if (*p == '+' || *p == '-')
        negative = *p++ == '-';

    double value = 0;
    const char* hexEnd = NULL;
    int n;

    if ((n = matchWord(p, "infinity")) || (n = matchWord(p, "inf")))
    {
        p += n;
        value = HUGE_VAL;
    }
    else if ((n = matchWord(p, "nan")))
    {
        p += n;
        if (*p == '(')
        {
            const char* q = p + 1;
            while (isalnum((unsigned char)*q) || *q == '_')
                q++;
            if (*q == ')')
                p = q + 1;
        }
        value = NAN;
    }
    else if (p[0] == '0' && (p[1] == 'x' || p[1] == 'X') && (hexEnd = scanHexFloat(p + 2, &value)))
    {
        p = hexEnd;
        if (isinf(value))
            errno = ERANGE;
    }
    else
    {
        const char* q = p;
        int digits = 0;
        while (isdigit((unsigned char)*q))
            q++, digits++;
        if (*q == '.')
        {
            q++;
            while (isdigit((unsigned char)*q))
                q++, digits++;
        }
        if (digits == 0)
        {
            if (end)
                *end = (char*)s;
            return 0;
        }
        if (*q == 'e' || *q == 'E')
        {
            const char* e = q + 1;
            if (*e == '+' || *e == '-')
                e++;
            if (isdigit((unsigned char)*e))
            {
                while (isdigit((unsigned char)*e))
                    e++;
                q = e;
            }
        }

        value = mlua_shim_parse_f64(numberStart, (size_t)(q - numberStart));
        if (isinf(value))
            errno = ERANGE;
        if (end)
            *end = (char*)q;
        return value;
    }

    if (end)
        *end = (char*)p;
    return negative ? -value : value;
}

static unsigned long long scanUnsigned(const char* s, char** end, int base, unsigned long long max)
{
    const char* p = s;
    while (isspace((unsigned char)*p))
        p++;

    int negative = 0;
    if (*p == '+' || *p == '-')
        negative = *p++ == '-';

    if ((base == 0 || base == 16) && p[0] == '0' && (p[1] == 'x' || p[1] == 'X') && hexDigit((unsigned char)p[2]) >= 0)
    {
        p += 2;
        base = 16;
    }
    else if (base == 0)
    {
        base = p[0] == '0' ? 8 : 10;
    }

    if (base < 2 || base > 36)
    {
        errno = EINVAL;
        if (end)
            *end = (char*)s;
        return 0;
    }

    unsigned long long value = 0;
    int digits = 0;
    int overflow = 0;
    for (;; p++)
    {
        int c = (unsigned char)*p;
        int d = isdigit(c) ? c - '0' : isalpha(c) ? tolower(c) - 'a' + 10 : 99;
        if (d >= base)
            break;
        digits++;
        if (value > (max - (unsigned)d) / (unsigned)base)
            overflow = 1;
        else
            value = value * (unsigned)base + (unsigned)d;
    }

    if (digits == 0)
    {
        if (end)
            *end = (char*)s;
        return 0;
    }
    if (end)
        *end = (char*)p;
    if (overflow)
    {
        errno = ERANGE;
        return max;
    }
    return negative ? (0 - value) & max : value;
}

unsigned long long strtoull(const char* s, char** end, int base)
{
    return scanUnsigned(s, end, base, ~0ull);
}

unsigned long strtoul(const char* s, char** end, int base)
{
    return (unsigned long)scanUnsigned(s, end, base, (unsigned long)~0ul);
}

int atoi(const char* s)
{
    return (int)scanUnsigned(s, NULL, 10, ~0ull);
}

struct mlua_shim_file
{
    int unused;
};

static struct mlua_shim_file streams[2];
FILE* const stdout = &streams[0];
FILE* const stderr = &streams[1];

static char lineBuffer[512];
static size_t lineLength;

static void flushLine(void)
{
    if (lineLength)
        mlua_shim_log(lineBuffer, lineLength);
    lineLength = 0;
}

static void putByte(char c)
{
    if (c == '\n')
    {
        flushLine();
        return;
    }
    if (lineLength == sizeof(lineBuffer))
        flushLine();
    lineBuffer[lineLength++] = c;
}

static void putBytes(const char* s, size_t n)
{
    for (size_t i = 0; i < n; i++)
        putByte(s[i]);
}

int vfprintf(FILE* f, const char* fmt, va_list args)
{
    (void)f;
    char buf[1024];
    va_list copy;
    va_copy(copy, args);
    int n = vsnprintf(buf, sizeof(buf), fmt, copy);
    va_end(copy);
    if (n < 0)
        return n;
    putBytes(buf, (size_t)n < sizeof(buf) ? (size_t)n : sizeof(buf) - 1);
    return n;
}

int fprintf(FILE* f, const char* fmt, ...)
{
    va_list args;
    va_start(args, fmt);
    int n = vfprintf(f, fmt, args);
    va_end(args);
    return n;
}

int printf(const char* fmt, ...)
{
    va_list args;
    va_start(args, fmt);
    int n = vfprintf(stdout, fmt, args);
    va_end(args);
    return n;
}

int fputc(int c, FILE* f)
{
    (void)f;
    putByte((char)c);
    return (unsigned char)c;
}

int fputs(const char* s, FILE* f)
{
    (void)f;
    putBytes(s, strlen(s));
    return 0;
}

int puts(const char* s)
{
    putBytes(s, strlen(s));
    putByte('\n');
    return 0;
}

size_t fwrite(const void* data, size_t size, size_t count, FILE* f)
{
    (void)f;
    putBytes((const char*)data, size * count);
    return count;
}

int fflush(FILE* f)
{
    (void)f;
    flushLine();
    return 0;
}

double difftime(time_t end, time_t start)
{
    return (double)(end - start);
}

time_t mktime(struct tm* tm)
{
    (void)tm;
    return -1;
}

struct tm* gmtime_r(const time_t* t, struct tm* out)
{
    (void)t;
    (void)out;
    return NULL;
}

struct tm* localtime_r(const time_t* t, struct tm* out)
{
    (void)t;
    (void)out;
    return NULL;
}

struct tm* localtime(const time_t* t)
{
    (void)t;
    return NULL;
}

size_t strftime(char* buf, size_t cap, const char* fmt, const struct tm* tm)
{
    (void)buf;
    (void)cap;
    (void)fmt;
    (void)tm;
    return 0;
}
