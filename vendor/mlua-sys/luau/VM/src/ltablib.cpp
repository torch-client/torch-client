#include "lualib.h"

#include "lapi.h"
#include "lnumutils.h"
#include "lstate.h"
#include "ltable.h"
#include "lstring.h"
#include "lgc.h"
#include "ldebug.h"
#include "lvm.h"

static int foreachi(lua_State* L)
{
    luaL_checktype(L, 1, LUA_TTABLE);
    luaL_checktype(L, 2, LUA_TFUNCTION);
    int i;
    int n = lua_objlen(L, 1);
    for (i = 1; i <= n; i++)
    {
        lua_pushvalue(L, 2);
        lua_pushinteger(L, i);
        lua_rawgeti(L, 1, i);
        lua_call(L, 2, 1);
        if (!lua_isnil(L, -1))
            return 1;
        lua_pop(L, 1);
    }
    return 0;
}

static int foreach (lua_State* L)
{
    luaL_checktype(L, 1, LUA_TTABLE);
    luaL_checktype(L, 2, LUA_TFUNCTION);
    lua_pushnil(L);
    while (lua_next(L, 1))
    {
        lua_pushvalue(L, 2);
        lua_pushvalue(L, -3);
        lua_pushvalue(L, -3);
        lua_call(L, 2, 1);
        if (!lua_isnil(L, -1))
            return 1;
        lua_pop(L, 2);
    }
    return 0;
}

static int maxn(lua_State* L)
{
    double max = 0;
    luaL_checktype(L, 1, LUA_TTABLE);

    LuaTable* t = hvalue(L->base);

    for (int i = 0; i < t->sizearray; i++)
    {
        if (!ttisnil(&t->array[i]))
            max = i + 1;
    }

    for (int i = 0; i < sizenode(t); i++)
    {
        LuaNode* n = gnode(t, i);

        if (!ttisnil(gval(n)) && ttisnumber(gkey(n)))
        {
            double v = nvalue(gkey(n));

            if (v > max)
                max = v;
        }
    }

    lua_pushnumber(L, max);
    return 1;
}

static int getn(lua_State* L)
{
    luaL_checktype(L, 1, LUA_TTABLE);
    lua_pushinteger(L, lua_objlen(L, 1));
    return 1;
}

static void moveelements(lua_State* L, int srct, int dstt, int f, int e, int t)
{
    LuaTable* src = hvalue(L->base + (srct - 1));
    LuaTable* dst = hvalue(L->base + (dstt - 1));

    if (dst->readonly)
        luaG_readonlyerror(L);

    int n = e - f + 1;

    if (unsigned(f) - 1 < unsigned(src->sizearray) && unsigned(t) - 1 < unsigned(dst->sizearray) &&
        unsigned(f) - 1 + unsigned(n) <= unsigned(src->sizearray) && unsigned(t) - 1 + unsigned(n) <= unsigned(dst->sizearray))
    {
        TValue* srcarray = src->array;
        TValue* dstarray = dst->array;

        if (t > e || t <= f || (dstt != srct && dst != src))
        {
            for (int i = 0; i < n; ++i)
            {
                TValue* s = &srcarray[f + i - 1];
                TValue* d = &dstarray[t + i - 1];
                setobj2t(L, d, s);
            }
        }
        else
        {
            for (int i = n - 1; i >= 0; i--)
            {
                TValue* s = &srcarray[(f + i) - 1];
                TValue* d = &dstarray[(t + i) - 1];
                setobj2t(L, d, s);
            }
        }

        luaC_barrierfast(L, dst);
    }
    else
    {
        if (t > e || t <= f || dst != src)
        {
            for (int i = 0; i < n; ++i)
            {
                lua_rawgeti(L, srct, f + i);
                lua_rawseti(L, dstt, t + i);
            }
        }
        else
        {
            for (int i = n - 1; i >= 0; i--)
            {
                lua_rawgeti(L, srct, f + i);
                lua_rawseti(L, dstt, t + i);
            }
        }
    }
}

static int tinsert(lua_State* L)
{
    luaL_checktype(L, 1, LUA_TTABLE);
    int n = lua_objlen(L, 1);
    int pos;
    switch (lua_gettop(L))
    {
    case 2:
    {
        pos = n + 1;
        break;
    }
    case 3:
    {
        pos = luaL_checkinteger(L, 2);

        if (1 <= pos && pos <= n)
            moveelements(L, 1, 1, pos, n, pos + 1);
        break;
    }
    default:
    {
        luaL_error(L, "wrong number of arguments to 'insert'");
    }
    }
    lua_rawseti(L, 1, pos);
    return 0;
}

static int tremove(lua_State* L)
{
    luaL_checktype(L, 1, LUA_TTABLE);
    int n = lua_objlen(L, 1);
    int pos = luaL_optinteger(L, 2, n);

    if (!(1 <= pos && pos <= n))
        return 0;
    lua_rawgeti(L, 1, pos);

    moveelements(L, 1, 1, pos + 1, n, pos);

    lua_pushnil(L);
    lua_rawseti(L, 1, n);
    return 1;
}

static int tmove(lua_State* L)
{
    luaL_checktype(L, 1, LUA_TTABLE);
    int f = luaL_checkinteger(L, 2);
    int e = luaL_checkinteger(L, 3);
    int t = luaL_checkinteger(L, 4);
    int tt = !lua_isnoneornil(L, 5) ? 5 : 1;
    luaL_checktype(L, tt, LUA_TTABLE);

    if (e >= f)
    {
        luaL_argcheck(L, f > 0 || e < INT_MAX + f, 3, "too many elements to move");
        int n = e - f + 1;
        luaL_argcheck(L, t <= INT_MAX - n + 1, 4, "destination wrap around");

        LuaTable* dst = hvalue(L->base + (tt - 1));

        if (dst->readonly)
            luaG_readonlyerror(L);

        if (t > 0 && (t - 1) <= dst->sizearray && (t - 1 + n) > dst->sizearray)
        {
            luaH_resizearray(L, dst, t - 1 + n);
        }

        moveelements(L, 1, tt, f, e, t);
    }
    lua_pushvalue(L, tt);
    return 1;
}

static void addfield(lua_State* L, luaL_Strbuf* b, int i, LuaTable* t)
{
    if (t && unsigned(i - 1) < unsigned(t->sizearray) && ttisstring(&t->array[i - 1]))
    {
        TString* ts = tsvalue(&t->array[i - 1]);
        luaL_addlstring(b, getstr(ts), ts->len);
    }
    else
    {
        int tt = lua_rawgeti(L, 1, i);
        if (tt != LUA_TSTRING && tt != LUA_TNUMBER)
            luaL_error(L, "invalid value (%s) at index %d in table for 'concat'", luaL_typename(L, -1), i);
        luaL_addvalue(b);
    }
}

static int tconcat(lua_State* L)
{
    size_t lsep;
    const char* sep = luaL_optlstring(L, 2, "", &lsep);
    luaL_checktype(L, 1, LUA_TTABLE);
    int i = luaL_optinteger(L, 3, 1);
    int last = luaL_opt(L, luaL_checkinteger, 4, lua_objlen(L, 1));

    LuaTable* t = hvalue(L->base);

    luaL_Strbuf b;
    luaL_buffinit(L, &b);
    for (; i < last; i++)
    {
        addfield(L, &b, i, t);
        if (lsep != 0)
            luaL_addlstring(&b, sep, lsep);
    }
    if (i == last)
        addfield(L, &b, i, t);
    luaL_pushresult(&b);
    return 1;
}

static int tpack(lua_State* L)
{
    int n = lua_gettop(L);
    lua_createtable(L, n, 1);

    LuaTable* t = hvalue(L->top - 1);

    for (int i = 0; i < n; ++i)
    {
        TValue* e = &t->array[i];
        setobj2t(L, e, L->base + i);
    }

    TValue* nv = luaH_setstr(L, t, luaS_newliteral(L, "n"));
    setnvalue(nv, n);

    return 1;
}

static int tunpack(lua_State* L)
{
    luaL_checktype(L, 1, LUA_TTABLE);
    LuaTable* t = hvalue(L->base);

    int i = luaL_optinteger(L, 2, 1);
    int e = luaL_opt(L, luaL_checkinteger, 3, lua_objlen(L, 1));
    if (i > e)
        return 0;
    unsigned n = (unsigned)e - i;
    if (n >= (unsigned int)INT_MAX || !lua_checkstack(L, (int)(++n)))
        luaL_error(L, "too many results to unpack");

    if (i == 1 && int(n) <= t->sizearray)
    {
        for (i = 0; i < int(n); i++)
            setobj2s(L, L->top + i, &t->array[i]);
        L->top += n;
    }
    else
    {
        for (; i < e; i++)
            lua_rawgeti(L, 1, i);
        lua_rawgeti(L, 1, e);
    }
    return (int)n;
}

typedef int (*SortPredicate)(lua_State* L, const TValue* l, const TValue* r);

static int sort_func(lua_State* L, const TValue* l, const TValue* r)
{
    LUAU_ASSERT(L->top == L->base + 2);

    setobj2s(L, L->top, &L->base[1]);
    setobj2s(L, L->top + 1, l);
    setobj2s(L, L->top + 2, r);
    L->top += 3;
    luaD_call(L, L->top - 3, 1);
    L->top -= 1;

    return !l_isfalse(L->top);
}

inline void sort_swap(lua_State* L, LuaTable* t, int i, int j)
{
    TValue* arr = t->array;
    int n = t->sizearray;
    LUAU_ASSERT(unsigned(i) < unsigned(n) && unsigned(j) < unsigned(n));

    TValue temp;
    setobj2s(L, &temp, &arr[i]);
    setobj2t(L, &arr[i], &arr[j]);
    setobj2t(L, &arr[j], &temp);
}

inline int sort_less(lua_State* L, LuaTable* t, int i, int j, SortPredicate pred)
{
    TValue* arr = t->array;
    int n = t->sizearray;
    LUAU_ASSERT(unsigned(i) < unsigned(n) && unsigned(j) < unsigned(n));

    int res = pred(L, &arr[i], &arr[j]);

    if (t->sizearray != n)
        luaL_error(L, "table modified during sorting");

    return res;
}

static void sort_siftheap(lua_State* L, LuaTable* t, int l, int u, SortPredicate pred, int root)
{
    LUAU_ASSERT(l <= u);
    int count = u - l + 1;

    while (root * 2 + 2 < count)
    {
        int left = root * 2 + 1, right = root * 2 + 2;
        int next = root;
        next = sort_less(L, t, l + next, l + left, pred) ? left : next;
        next = sort_less(L, t, l + next, l + right, pred) ? right : next;

        if (next == root)
            break;

        sort_swap(L, t, l + root, l + next);
        root = next;
    }

    int lastleft = root * 2 + 1;
    if (lastleft == count - 1 && sort_less(L, t, l + root, l + lastleft, pred))
        sort_swap(L, t, l + root, l + lastleft);
}

static void sort_heap(lua_State* L, LuaTable* t, int l, int u, SortPredicate pred)
{
    LUAU_ASSERT(l <= u);
    int count = u - l + 1;

    for (int i = count / 2 - 1; i >= 0; --i)
        sort_siftheap(L, t, l, u, pred, i);

    for (int i = count - 1; i > 0; --i)
    {
        sort_swap(L, t, l, l + i);
        sort_siftheap(L, t, l, l + i - 1, pred, 0);
    }
}

static void sort_rec(lua_State* L, LuaTable* t, int l, int u, int limit, SortPredicate pred)
{
    while (l < u)
    {
        if (limit == 0)
            return sort_heap(L, t, l, u, pred);

        if (sort_less(L, t, u, l, pred))
            sort_swap(L, t, u, l);
        if (u - l == 1)
            break;
        int m = l + ((u - l) >> 1);
        if (sort_less(L, t, m, l, pred))
            sort_swap(L, t, m, l);
        else if (sort_less(L, t, u, m, pred))
            sort_swap(L, t, m, u);
        if (u - l == 2)
            break;

        int p = u - 1;
        sort_swap(L, t, m, u - 1);

        int i = l;
        int j = u - 1;
        for (;;)
        {
            while (sort_less(L, t, ++i, p, pred))
            {
                if (i >= u)
                    luaL_error(L, "invalid order function for sorting");
            }
            while (sort_less(L, t, p, --j, pred))
            {
                if (j <= l)
                    luaL_error(L, "invalid order function for sorting");
            }
            if (j < i)
                break;
            sort_swap(L, t, i, j);
        }

        sort_swap(L, t, p, i);

        limit = (limit >> 1) + (limit >> 2);

        if (i - l < u - i)
        {
            sort_rec(L, t, l, i - 1, limit, pred);
            l = i + 1;
        }
        else
        {
            sort_rec(L, t, i + 1, u, limit, pred);
            u = i - 1;
        }
    }
}

static int tsort(lua_State* L)
{
    luaL_checktype(L, 1, LUA_TTABLE);
    LuaTable* t = hvalue(L->base);
    int n = luaH_getn(t);
    if (t->readonly)
        luaG_readonlyerror(L);

    SortPredicate pred = luaV_lessthan;
    if (!lua_isnoneornil(L, 2))
    {
        luaL_checktype(L, 2, LUA_TFUNCTION);
        pred = sort_func;
    }
    lua_settop(L, 2);

    if (n > 0)
        sort_rec(L, t, 0, n - 1, n, pred);
    return 0;
}

static int tcreate(lua_State* L)
{
    int size = luaL_checkinteger(L, 1);
    if (size < 0)
        luaL_argerror(L, 1, "size out of range");

    if (!lua_isnoneornil(L, 2))
    {
        lua_createtable(L, size, 0);
        LuaTable* t = hvalue(L->top - 1);

        StkId v = L->base + 1;

        for (int i = 0; i < size; ++i)
        {
            TValue* e = &t->array[i];
            setobj2t(L, e, v);
        }
    }
    else
    {
        lua_createtable(L, size, 0);
    }

    return 1;
}

static int tfind(lua_State* L)
{
    luaL_checktype(L, 1, LUA_TTABLE);
    luaL_checkany(L, 2);
    int init = luaL_optinteger(L, 3, 1);
    if (init < 1)
        luaL_argerror(L, 3, "index out of range");

    LuaTable* t = hvalue(L->base);

    for (int i = init;; ++i)
    {
        const TValue* e = luaH_getnum(t, i);
        if (ttisnil(e))
            break;

        StkId v = L->base + 1;

        if (equalobj(L, v, e))
        {
            lua_pushinteger(L, i);
            return 1;
        }
    }

    lua_pushnil(L);
    return 1;
}

static int tclear(lua_State* L)
{
    luaL_checktype(L, 1, LUA_TTABLE);

    LuaTable* tt = hvalue(L->base);
    if (tt->readonly)
        luaG_readonlyerror(L);

    luaH_clear(tt);
    return 0;
}

static int tfreeze(lua_State* L)
{
    luaL_checktype(L, 1, LUA_TTABLE);
    luaL_argcheck(L, !lua_getreadonly(L, 1), 1, "table is already frozen");
    luaL_argcheck(L, !luaL_getmetafield(L, 1, "__metatable"), 1, "table has a protected metatable");

    lua_setreadonly(L, 1, true);

    lua_pushvalue(L, 1);
    return 1;
}

static int tisfrozen(lua_State* L)
{
    luaL_checktype(L, 1, LUA_TTABLE);

    lua_pushboolean(L, lua_getreadonly(L, 1));
    return 1;
}

static int tclone(lua_State* L)
{
    luaL_checktype(L, 1, LUA_TTABLE);
    luaL_argcheck(L, !luaL_getmetafield(L, 1, "__metatable"), 1, "table has a protected metatable");

    LuaTable* tt = luaH_clone(L, hvalue(L->base));

    TValue v;
    sethvalue(L, &v, tt);
    luaA_pushobject(L, &v);

    return 1;
}

static const luaL_Reg tab_funcs[] = {
    {"concat", tconcat},
    {"foreach", foreach},
    {"foreachi", foreachi},
    {"getn", getn},
    {"maxn", maxn},
    {"insert", tinsert},
    {"remove", tremove},
    {"sort", tsort},
    {"pack", tpack},
    {"unpack", tunpack},
    {"move", tmove},
    {"create", tcreate},
    {"find", tfind},
    {"clear", tclear},
    {"freeze", tfreeze},
    {"isfrozen", tisfrozen},
    {"clone", tclone},
    {NULL, NULL},
};

int luaopen_table(lua_State* L)
{
    luaL_register(L, LUA_TABLIBNAME, tab_funcs);

    lua_pushcfunction(L, tunpack, "unpack");
    lua_setglobal(L, "unpack");

    return 1;
}
