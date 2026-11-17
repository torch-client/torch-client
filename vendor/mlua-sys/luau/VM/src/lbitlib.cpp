#include "lualib.h"

#include "lcommon.h"
#include "lnumutils.h"

#define ALLONES ~0u
#define NBITS int(8 * sizeof(unsigned))

#define trim(x) ((x) & ALLONES)

#define mask(n) (~((ALLONES << 1) << ((n)-1)))

typedef unsigned b_uint;

static b_uint andaux(lua_State* L)
{
    int i, n = lua_gettop(L);
    b_uint r = ~(b_uint)0;
    for (i = 1; i <= n; i++)
        r &= luaL_checkunsigned(L, i);
    return trim(r);
}

static int b_and(lua_State* L)
{
    b_uint r = andaux(L);
    lua_pushunsigned(L, r);
    return 1;
}

static int b_test(lua_State* L)
{
    b_uint r = andaux(L);
    lua_pushboolean(L, r != 0);
    return 1;
}

static int b_or(lua_State* L)
{
    int i, n = lua_gettop(L);
    b_uint r = 0;
    for (i = 1; i <= n; i++)
        r |= luaL_checkunsigned(L, i);
    lua_pushunsigned(L, trim(r));
    return 1;
}

static int b_xor(lua_State* L)
{
    int i, n = lua_gettop(L);
    b_uint r = 0;
    for (i = 1; i <= n; i++)
        r ^= luaL_checkunsigned(L, i);
    lua_pushunsigned(L, trim(r));
    return 1;
}

static int b_not(lua_State* L)
{
    b_uint r = ~luaL_checkunsigned(L, 1);
    lua_pushunsigned(L, trim(r));
    return 1;
}

static int b_shift(lua_State* L, b_uint r, int i)
{
    if (i < 0)
    {
        i = -i;
        r = trim(r);
        if (i >= NBITS)
            r = 0;
        else
            r >>= i;
    }
    else
    {
        if (i >= NBITS)
            r = 0;
        else
            r <<= i;
        r = trim(r);
    }
    lua_pushunsigned(L, r);
    return 1;
}

static int b_lshift(lua_State* L)
{
    return b_shift(L, luaL_checkunsigned(L, 1), luaL_checkinteger(L, 2));
}

static int b_rshift(lua_State* L)
{
    return b_shift(L, luaL_checkunsigned(L, 1), -luaL_checkinteger(L, 2));
}

static int b_arshift(lua_State* L)
{
    b_uint r = luaL_checkunsigned(L, 1);
    int i = luaL_checkinteger(L, 2);
    if (i < 0 || !(r & ((b_uint)1 << (NBITS - 1))))
        return b_shift(L, r, -i);
    else
    {
        if (i >= NBITS)
            r = ALLONES;
        else
            r = trim((r >> i) | ~(~(b_uint)0 >> i));
        lua_pushunsigned(L, r);
        return 1;
    }
}

static int b_rot(lua_State* L, int i)
{
    b_uint r = luaL_checkunsigned(L, 1);
    i &= (NBITS - 1);
    r = trim(r);
    if (i != 0)
        r = (r << i) | (r >> (NBITS - i));
    lua_pushunsigned(L, trim(r));
    return 1;
}

static int b_lrot(lua_State* L)
{
    return b_rot(L, luaL_checkinteger(L, 2));
}

static int b_rrot(lua_State* L)
{
    return b_rot(L, -luaL_checkinteger(L, 2));
}

static int fieldargs(lua_State* L, int farg, int* width)
{
    int f = luaL_checkinteger(L, farg);
    int w = luaL_optinteger(L, farg + 1, 1);
    luaL_argcheck(L, 0 <= f, farg, "field cannot be negative");
    luaL_argcheck(L, 0 < w, farg + 1, "width must be positive");
    if (f + w > NBITS)
        luaL_error(L, "trying to access non-existent bits");
    *width = w;
    return f;
}

static int b_extract(lua_State* L)
{
    int w;
    b_uint r = luaL_checkunsigned(L, 1);
    int f = fieldargs(L, 2, &w);
    r = (r >> f) & mask(w);
    lua_pushunsigned(L, r);
    return 1;
}

static int b_replace(lua_State* L)
{
    int w;
    b_uint r = luaL_checkunsigned(L, 1);
    b_uint v = luaL_checkunsigned(L, 2);
    int f = fieldargs(L, 3, &w);
    int m = mask(w);
    v &= m;
    r = (r & ~(m << f)) | (v << f);
    lua_pushunsigned(L, r);
    return 1;
}

static int b_countlz(lua_State* L)
{
    b_uint v = luaL_checkunsigned(L, 1);

    b_uint r = NBITS;
    for (int i = 0; i < NBITS; ++i)
        if (v & (1u << (NBITS - 1 - i)))
        {
            r = i;
            break;
        }

    lua_pushunsigned(L, r);
    return 1;
}

static int b_countrz(lua_State* L)
{
    b_uint v = luaL_checkunsigned(L, 1);

    b_uint r = NBITS;
    for (int i = 0; i < NBITS; ++i)
        if (v & (1u << i))
        {
            r = i;
            break;
        }

    lua_pushunsigned(L, r);
    return 1;
}

static int b_swap(lua_State* L)
{
    b_uint n = luaL_checkunsigned(L, 1);
    n = (n << 24) | ((n << 8) & 0xff0000) | ((n >> 8) & 0xff00) | (n >> 24);

    lua_pushunsigned(L, n);
    return 1;
}

static const luaL_Reg bitlib[] = {
    {"arshift", b_arshift},
    {"band", b_and},
    {"bnot", b_not},
    {"bor", b_or},
    {"bxor", b_xor},
    {"btest", b_test},
    {"extract", b_extract},
    {"lrotate", b_lrot},
    {"lshift", b_lshift},
    {"replace", b_replace},
    {"rrotate", b_rrot},
    {"rshift", b_rshift},
    {"countlz", b_countlz},
    {"countrz", b_countrz},
    {"byteswap", b_swap},
    {NULL, NULL},
};

int luaopen_bit32(lua_State* L)
{
    luaL_register(L, LUA_BITLIBNAME, bitlib);

    return 1;
}
