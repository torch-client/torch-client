#include <stdarg.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

enum
{
    FLAG_LEFT = 1,
    FLAG_PLUS = 2,
    FLAG_SPACE = 4,
    FLAG_ALT = 8,
    FLAG_ZERO = 16
};

int mlua_shim_format_f64(char* buf, size_t cap, unsigned char conv, unsigned flags, int width, int prec, double value);

enum Length
{
    LEN_NONE,
    LEN_HH,
    LEN_H,
    LEN_L,
    LEN_LL,
    LEN_J,
    LEN_Z,
    LEN_T,
    LEN_BIG_L
};

typedef struct
{
    char* buf;
    size_t cap;
    size_t len;
} Out;

static void put(Out* out, char c)
{
    if (out->len < out->cap)
        out->buf[out->len] = c;
    out->len++;
}

static void putMany(Out* out, const char* s, size_t n)
{
    for (size_t i = 0; i < n; i++)
        put(out, s[i]);
}

static void pad(Out* out, char c, int n)
{
    for (; n > 0; n--)
        put(out, c);
}

static void putNumber(Out* out, const char* prefix, const char* digits, size_t ndigits, unsigned flags, int width, int prec)
{
    size_t nprefix = strlen(prefix);
    int zeros = prec > (int)ndigits ? prec - (int)ndigits : 0;
    int body = (int)nprefix + zeros + (int)ndigits;

    if ((flags & FLAG_ZERO) && !(flags & FLAG_LEFT) && prec < 0 && width > body)
    {
        zeros += width - body;
        body = width;
    }

    if (!(flags & FLAG_LEFT))
        pad(out, ' ', width - body);
    putMany(out, prefix, nprefix);
    pad(out, '0', zeros);
    putMany(out, digits, ndigits);
    if (flags & FLAG_LEFT)
        pad(out, ' ', width - body);
}

static void putInteger(Out* out, char conv, uintmax_t value, int negative, unsigned flags, int width, int prec)
{
    char digits[32];
    char* end = digits + sizeof(digits);
    char* p = end;
    unsigned base = conv == 'o' ? 8 : (conv == 'x' || conv == 'X' || conv == 'p') ? 16 : 10;
    const char* hex = conv == 'X' ? "0123456789ABCDEF" : "0123456789abcdef";

    while (value)
    {
        *--p = hex[value % base];
        value /= base;
    }

    char prefix[3] = {0};
    if (conv == 'd' || conv == 'i')
    {
        if (negative)
            prefix[0] = '-';
        else if (flags & FLAG_PLUS)
            prefix[0] = '+';
        else if (flags & FLAG_SPACE)
            prefix[0] = ' ';
    }
    else if (conv == 'p' || ((flags & FLAG_ALT) && (conv == 'x' || conv == 'X') && p != end))
    {
        prefix[0] = '0';
        prefix[1] = conv == 'X' ? 'X' : 'x';
    }

    if (prec < 0 && p == end)
        *--p = '0';

    if (conv == 'o' && (flags & FLAG_ALT) && (p == end || *p != '0'))
    {
        if (prec <= (int)(end - p))
            *--p = '0';
    }

    putNumber(out, prefix, p, (size_t)(end - p), flags, width, prec);
}

int vsnprintf(char* buf, size_t cap, const char* fmt, va_list args)
{
    Out out = {buf, cap, 0};

    for (const char* f = fmt; *f; f++)
    {
        if (*f != '%')
        {
            put(&out, *f);
            continue;
        }
        f++;

        unsigned flags = 0;
        for (;; f++)
        {
            if (*f == '-')
                flags |= FLAG_LEFT;
            else if (*f == '+')
                flags |= FLAG_PLUS;
            else if (*f == ' ')
                flags |= FLAG_SPACE;
            else if (*f == '#')
                flags |= FLAG_ALT;
            else if (*f == '0')
                flags |= FLAG_ZERO;
            else
                break;
        }

        int width = 0;
        if (*f == '*')
        {
            width = va_arg(args, int);
            if (width < 0)
            {
                flags |= FLAG_LEFT;
                width = width == INT32_MIN ? 0 : -width;
            }
            f++;
        }
        else
        {
            for (; *f >= '0' && *f <= '9'; f++)
                width = width < 100000000 ? width * 10 + (*f - '0') : width;
        }

        int prec = -1;
        if (*f == '.')
        {
            f++;
            prec = 0;
            if (*f == '*')
            {
                prec = va_arg(args, int);
                if (prec < 0)
                    prec = -1;
                f++;
            }
            else
            {
                for (; *f >= '0' && *f <= '9'; f++)
                    prec = prec < 100000000 ? prec * 10 + (*f - '0') : prec;
            }
        }

        enum Length length = LEN_NONE;
        switch (*f)
        {
        case 'h':
            length = f[1] == 'h' ? LEN_HH : LEN_H;
            f += length == LEN_HH ? 2 : 1;
            break;
        case 'l':
            length = f[1] == 'l' ? LEN_LL : LEN_L;
            f += length == LEN_LL ? 2 : 1;
            break;
        case 'j':
            length = LEN_J;
            f++;
            break;
        case 'z':
            length = LEN_Z;
            f++;
            break;
        case 't':
            length = LEN_T;
            f++;
            break;
        case 'L':
            length = LEN_BIG_L;
            f++;
            break;
        default:
            break;
        }

        char conv = *f;
        switch (conv)
        {
        case 'd':
        case 'i':
        {
            intmax_t v;
            switch (length)
            {
            case LEN_HH:
                v = (signed char)va_arg(args, int);
                break;
            case LEN_H:
                v = (short)va_arg(args, int);
                break;
            case LEN_L:
                v = va_arg(args, long);
                break;
            case LEN_LL:
            case LEN_BIG_L:
                v = va_arg(args, long long);
                break;
            case LEN_J:
                v = va_arg(args, intmax_t);
                break;
            case LEN_Z:
            case LEN_T:
                v = va_arg(args, ptrdiff_t);
                break;
            default:
                v = va_arg(args, int);
                break;
            }
            uintmax_t magnitude = v < 0 ? 0 - (uintmax_t)v : (uintmax_t)v;
            putInteger(&out, conv, magnitude, v < 0, flags, width, prec);
            break;
        }
        case 'u':
        case 'o':
        case 'x':
        case 'X':
        {
            uintmax_t v;
            switch (length)
            {
            case LEN_HH:
                v = (unsigned char)va_arg(args, unsigned);
                break;
            case LEN_H:
                v = (unsigned short)va_arg(args, unsigned);
                break;
            case LEN_L:
                v = va_arg(args, unsigned long);
                break;
            case LEN_LL:
            case LEN_BIG_L:
                v = va_arg(args, unsigned long long);
                break;
            case LEN_J:
                v = va_arg(args, uintmax_t);
                break;
            case LEN_Z:
            case LEN_T:
                v = va_arg(args, size_t);
                break;
            default:
                v = va_arg(args, unsigned);
                break;
            }
            putInteger(&out, conv, v, 0, flags, width, prec);
            break;
        }
        case 'p':
            putInteger(&out, 'p', (uintptr_t)va_arg(args, void*), 0, flags & FLAG_LEFT, width, -1);
            break;
        case 'c':
        {
            char c = (char)va_arg(args, int);
            putNumber(&out, "", &c, 1, flags & FLAG_LEFT, width, -1);
            break;
        }
        case 's':
        {
            const char* s = va_arg(args, const char*);
            if (!s)
                s = "(null)";
            size_t n = 0;
            while (s[n] && (prec < 0 || n < (size_t)prec))
                n++;
            putNumber(&out, "", s, n, flags & FLAG_LEFT, width, -1);
            break;
        }
        case 'e':
        case 'E':
        case 'f':
        case 'F':
        case 'g':
        case 'G':
        case 'a':
        case 'A':
        {
            if (length == LEN_BIG_L)
            {
                (void)va_arg(args, long double);
                break;
            }
            double v = va_arg(args, double);
            char* dst = out.len < out.cap ? out.buf + out.len : NULL;
            size_t room = out.len < out.cap ? out.cap - out.len : 0;
            int n = mlua_shim_format_f64(dst, room, (unsigned char)conv, flags, width, prec, v);
            out.len += (size_t)n;
            break;
        }
        case 'n':
            switch (length)
            {
            case LEN_HH:
                *va_arg(args, signed char*) = (signed char)out.len;
                break;
            case LEN_H:
                *va_arg(args, short*) = (short)out.len;
                break;
            case LEN_L:
                *va_arg(args, long*) = (long)out.len;
                break;
            case LEN_LL:
                *va_arg(args, long long*) = (long long)out.len;
                break;
            case LEN_J:
                *va_arg(args, intmax_t*) = (intmax_t)out.len;
                break;
            case LEN_Z:
            case LEN_T:
                *va_arg(args, ptrdiff_t*) = (ptrdiff_t)out.len;
                break;
            default:
                *va_arg(args, int*) = (int)out.len;
                break;
            }
            break;
        case '%':
            put(&out, '%');
            break;
        default:
            if (!conv)
                f--;
            else
            {
                put(&out, '%');
                put(&out, conv);
            }
            break;
        }
    }

    if (cap)
        buf[out.len < cap ? out.len : cap - 1] = 0;
    return out.len > 0x7fffffff ? -1 : (int)out.len;
}

int snprintf(char* buf, size_t cap, const char* fmt, ...)
{
    va_list args;
    va_start(args, fmt);
    int n = vsnprintf(buf, cap, fmt, args);
    va_end(args);
    return n;
}
