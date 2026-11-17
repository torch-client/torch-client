#include "lualib.h"

#include "lcommon.h"

#define MAXUNICODE 0x10FFFF

#define iscont(p) ((*(p) & 0xC0) == 0x80)

static int u_posrelat(int pos, size_t len)
{
    if (pos >= 0)
        return pos;
    else if (0u - (size_t)pos > len)
        return 0;
    else
        return (int)len + pos + 1;
}

static const char* utf8_decode(const char* o, int* val)
{
    static const unsigned int limits[] = {0xFF, 0x7F, 0x7FF, 0xFFFF};
    const unsigned char* s = (const unsigned char*)o;
    unsigned int c = s[0];
    unsigned int res = 0;
    if (c < 0x80)
        res = c;
    else
    {
        int count = 0;
        while (c & 0x40)
        {
            int cc = s[++count];
            if ((cc & 0xC0) != 0x80)
                return NULL;
            res = (res << 6) | (cc & 0x3F);
            c <<= 1;
        }
        res |= ((c & 0x7F) << (count * 5));
        if (count > 3 || res > MAXUNICODE || res <= limits[count])
            return NULL;
        if (unsigned(res - 0xD800) < 0x800)
            return NULL;
        s += count;
    }
    if (val)
        *val = res;
    return (const char*)s + 1;
}

static int utflen(lua_State* L)
{
    int n = 0;
    size_t len;
    const char* s = luaL_checklstring(L, 1, &len);
    int posi = u_posrelat(luaL_optinteger(L, 2, 1), len);
    int posj = u_posrelat(luaL_optinteger(L, 3, -1), len);
    luaL_argcheck(L, 1 <= posi && --posi <= (int)len, 2, "initial position out of string");
    luaL_argcheck(L, --posj < (int)len, 3, "final position out of string");
    while (posi <= posj)
    {
        const char* s1 = utf8_decode(s + posi, NULL);
        if (s1 == NULL)
        {
            lua_pushnil(L);
            lua_pushinteger(L, posi + 1);
            return 2;
        }
        posi = (int)(s1 - s);
        n++;
    }
    lua_pushinteger(L, n);
    return 1;
}

static int codepoint(lua_State* L)
{
    size_t len;
    const char* s = luaL_checklstring(L, 1, &len);
    int posi = u_posrelat(luaL_optinteger(L, 2, 1), len);
    int pose = u_posrelat(luaL_optinteger(L, 3, posi), len);
    int n;
    const char* se;
    luaL_argcheck(L, posi >= 1, 2, "out of range");
    luaL_argcheck(L, pose <= (int)len, 3, "out of range");
    if (posi > pose)
        return 0;
    if (pose - posi >= INT_MAX)
        luaL_error(L, "string slice too long");
    n = (int)(pose - posi) + 1;
    luaL_checkstack(L, n, "string slice too long");
    n = 0;
    se = s + pose;
    for (s += posi - 1; s < se;)
    {
        int code;
        s = utf8_decode(s, &code);
        if (s == NULL)
            luaL_error(L, "invalid UTF-8 code");
        lua_pushinteger(L, code);
        n++;
    }
    return n;
}

#define UTF8BUFFSZ 8

static int luaO_utf8esc(char* buff, unsigned long x)
{
    int n = 1;
    LUAU_ASSERT(x <= 0x10FFFF);
    if (x < 0x80)
        buff[UTF8BUFFSZ - 1] = cast_to(char, x);
    else
    {
        unsigned int mfb = 0x3f;
        do
        {
            buff[UTF8BUFFSZ - (n++)] = cast_to(char, 0x80 | (x & 0x3f));
            x >>= 6;
            mfb >>= 1;
        } while (x > mfb);
        buff[UTF8BUFFSZ - n] = cast_to(char, (~mfb << 1) | x);
    }
    return n;
}

static int buffutfchar(lua_State* L, int arg, char* buff, const char** charstr)
{
    int code = luaL_checkinteger(L, arg);
    luaL_argcheck(L, 0 <= code && code <= MAXUNICODE, arg, "value out of range");
    int l = luaO_utf8esc(buff, cast_to(long, code));
    *charstr = buff + UTF8BUFFSZ - l;
    return l;
}

static int utfchar(lua_State* L)
{
    char buff[UTF8BUFFSZ];
    const char* charstr;

    int n = lua_gettop(L);
    if (n == 1)
    {
        int l = buffutfchar(L, 1, buff, &charstr);
        lua_pushlstring(L, charstr, l);
    }
    else
    {
        luaL_Strbuf b;
        luaL_buffinit(L, &b);
        for (int i = 1; i <= n; i++)
        {
            int l = buffutfchar(L, i, buff, &charstr);
            luaL_addlstring(&b, charstr, l);
        }
        luaL_pushresult(&b);
    }
    return 1;
}

static int byteoffset(lua_State* L)
{
    size_t len;
    const char* s = luaL_checklstring(L, 1, &len);
    int n = luaL_checkinteger(L, 2);
    int posi = (n >= 0) ? 1 : (int)len + 1;
    posi = u_posrelat(luaL_optinteger(L, 3, posi), len);
    luaL_argcheck(L, 1 <= posi && --posi <= (int)len, 3, "position out of range");
    if (n == 0)
    {
        while (posi > 0 && iscont(s + posi))
            posi--;
    }
    else
    {
        if (iscont(s + posi))
            luaL_error(L, "initial position is a continuation byte");
        if (n < 0)
        {
            while (n < 0 && posi > 0)
            {
                do
                {
                    posi--;
                } while (posi > 0 && iscont(s + posi));
                n++;
            }
        }
        else
        {
            n--;
            while (n > 0 && posi < (int)len)
            {
                do
                {
                    posi++;
                } while (iscont(s + posi));
                n--;
            }
        }
    }
    if (n == 0)
        lua_pushinteger(L, posi + 1);
    else
        lua_pushnil(L);
    return 1;
}

static int iter_aux(lua_State* L)
{
    size_t len;
    const char* s = luaL_checklstring(L, 1, &len);
    int n = lua_tointeger(L, 2) - 1;
    if (n < 0)
        n = 0;
    else if (n < (int)len)
    {
        n++;
        while (iscont(s + n))
            n++;
    }
    if (n >= (int)len)
        return 0;
    else
    {
        int code;
        const char* next = utf8_decode(s + n, &code);
        if (next == NULL || iscont(next))
            luaL_error(L, "invalid UTF-8 code");
        lua_pushinteger(L, n + 1);
        lua_pushinteger(L, code);
        return 2;
    }
}

static int iter_codes(lua_State* L)
{
    luaL_checkstring(L, 1);
    lua_pushcfunction(L, iter_aux, NULL);
    lua_pushvalue(L, 1);
    lua_pushinteger(L, 0);
    return 3;
}

#define UTF8PATT "[\0-\x7F\xC2-\xF4][\x80-\xBF]*"

static const luaL_Reg funcs[] = {
    {"offset", byteoffset},
    {"codepoint", codepoint},
    {"char", utfchar},
    {"len", utflen},
    {"codes", iter_codes},
    {NULL, NULL},
};

int luaopen_utf8(lua_State* L)
{
    luaL_register(L, LUA_UTF8LIBNAME, funcs);

    lua_pushlstring(L, UTF8PATT, sizeof(UTF8PATT) / sizeof(char) - 1);
    lua_setfield(L, -2, "charpattern");

    return 1;
}
