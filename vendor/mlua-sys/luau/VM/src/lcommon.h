#pragma once

#include <limits.h>
#include <stdint.h>

#include "luaconf.h"

#include "Luau/Common.h"

#define check_exp(c, e) (LUAU_ASSERT(c), (e))
#define api_check(l, e) LUAU_ASSERT(e)

#ifndef cast_to
#define cast_to(t, exp) ((t)(exp))
#endif

#define cast_byte(i) cast_to(uint8_t, (i))
#define cast_num(i) cast_to(double, (i))
#define cast_int(i) cast_to(int, (i))

typedef uint32_t Instruction;

#if defined(HARDSTACKTESTS) && HARDSTACKTESTS
#define condhardstacktests(x) (x)
#else
#define condhardstacktests(x) ((void)0)
#endif

#if defined(HARDMEMTESTS) && HARDMEMTESTS
#define condhardmemtests(x, l) (HARDMEMTESTS >= l ? (x) : (void)0)
#else
#define condhardmemtests(x, l) ((void)0)
#endif
