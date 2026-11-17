#pragma once

#if defined(_MSC_VER) && !defined(__clang__)
#define LUAU_FASTMATH_BEGIN __pragma(float_control(precise, off, push))
#define LUAU_FASTMATH_END __pragma(float_control(pop))
#else
#define LUAU_FASTMATH_BEGIN
#define LUAU_FASTMATH_END
#endif

#if (defined(__x86_64__) || defined(_M_X64)) && !defined(__SSE4_1__) && !defined(__AVX__)
#if defined(_MSC_VER) && !defined(__clang__)
#define LUAU_TARGET_SSE41
#elif defined(__GNUC__) && defined(__has_attribute)
#if __has_attribute(target)
#define LUAU_TARGET_SSE41 __attribute__((target("sse4.1")))
#endif
#endif
#endif

#if defined(__GNUC__)
#define LUA_PRINTF_ATTR(fmt, arg) __attribute__((format(printf, fmt, arg)))
#else
#define LUA_PRINTF_ATTR(fmt, arg)
#endif

#ifdef _MSC_VER
#define LUA_NORETURN __declspec(noreturn)
#else
#define LUA_NORETURN __attribute__((__noreturn__))
#endif

#ifndef LUA_API
#define LUA_API extern
#endif

#define LUALIB_API LUA_API

#if defined(__GNUC__)
#define LUAI_FUNC __attribute__((visibility("hidden"))) extern
#define LUAI_DATA LUAI_FUNC
#else
#define LUAI_FUNC extern
#define LUAI_DATA extern
#endif

#ifndef LUA_USE_LONGJMP
#define LUA_USE_LONGJMP 0
#endif

#ifndef LUA_IDSIZE
#define LUA_IDSIZE 256
#endif

#ifndef LUA_MINSTACK
#define LUA_MINSTACK 20
#endif

#ifndef LUAI_MAXCSTACK
#define LUAI_MAXCSTACK 8000
#endif

#ifndef LUAI_MAXCALLS
#define LUAI_MAXCALLS 20000
#endif

#ifndef LUAI_MAXCCALLS
#define LUAI_MAXCCALLS 200
#endif

#ifndef LUA_BUFFERSIZE
#define LUA_BUFFERSIZE 512
#endif

#ifndef LUA_UTAG_LIMIT
#define LUA_UTAG_LIMIT 128
#endif

#ifndef LUA_LUTAG_LIMIT
#define LUA_LUTAG_LIMIT 128
#endif

#ifndef LUA_SIZECLASSES
#define LUA_SIZECLASSES 40
#endif

#ifndef LUA_MEMORY_CATEGORIES
#define LUA_MEMORY_CATEGORIES 256
#endif

#ifndef LUA_EXECUTION_CALLBACK_STORAGE
#define LUA_EXECUTION_CALLBACK_STORAGE 512
#endif

#ifndef LUA_MINSTRTABSIZE
#define LUA_MINSTRTABSIZE 32
#endif

#ifndef LUA_MAXCAPTURES
#define LUA_MAXCAPTURES 32
#endif

#ifndef LUA_VECTOR_SIZE
#define LUA_VECTOR_SIZE 3
#endif

#define LUA_EXTRA_SIZE (LUA_VECTOR_SIZE - 2)
