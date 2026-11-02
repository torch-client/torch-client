#include <chrono>
#include <exception>
#include <new>
#include <string>

#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <time.h>

template class std::basic_string<char>;
template std::string std::operator+ <char, std::char_traits<char>, std::allocator<char>>(const char*, const std::string&);

_LIBCPP_BEGIN_NAMESPACE_STD

void __libcpp_verbose_abort(const char* format, ...)
{
    va_list args;
    va_start(args, format);
    char message[256];
    vsnprintf(message, sizeof(message), format, args);
    va_end(args);
    fputs(message, stderr);
    fputc('\n', stderr);
    abort();
}

string to_string(int value)
{
    char buf[16];
    int n = snprintf(buf, sizeof(buf), "%d", value);
    return string(buf, size_t(n));
}

string to_string(double value)
{
    char buf[320];
    int n = snprintf(buf, sizeof(buf), "%f", value);
    return string(buf, size_t(n));
}

namespace chrono
{

steady_clock::time_point steady_clock::now() noexcept
{
    return time_point(duration_cast<duration>(microseconds(clock())));
}

}

_LIBCPP_END_NAMESPACE_STD

std::exception::~exception() noexcept {}

const char* std::exception::what() const noexcept
{
    return "std::exception";
}

void* operator new(size_t size)
{
    void* p = malloc(size ? size : 1);
    if (!p)
        abort();
    return p;
}

void* operator new[](size_t size)
{
    return ::operator new(size);
}

void operator delete(void* p) noexcept
{
    free(p);
}

void operator delete[](void* p) noexcept
{
    free(p);
}

void operator delete(void* p, size_t) noexcept
{
    free(p);
}

void operator delete[](void* p, size_t) noexcept
{
    free(p);
}

extern "C"
{
    void* __dso_handle = &__dso_handle;

    int __cxa_atexit(void (*)(void*), void*, void*)
    {
        return 0;
    }

    void __cxa_pure_virtual()
    {
        abort();
    }
}
