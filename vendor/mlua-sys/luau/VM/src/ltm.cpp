#include "ltm.h"

#include "lstate.h"
#include "lstring.h"
#include "ludata.h"
#include "ltable.h"
#include "lgc.h"

#include <string.h>

const char* const luaT_typenames[] = {
    "nil",
    "boolean",

    "userdata",
    "number",
    "vector",

    "string",

    "table",
    "function",
    "userdata",
    "thread",
    "buffer",
};

const char* const luaT_eventname[] = {
    "__index",
    "__newindex",
    "__mode",
    "__namecall",
    "__call",
    "__iter",
    "__len",

    "__eq",

    "__add",
    "__sub",
    "__mul",
    "__div",
    "__idiv",
    "__mod",
    "__pow",
    "__unm",

    "__lt",
    "__le",
    "__concat",
    "__type",
    "__metatable",
};

static_assert(sizeof(luaT_typenames) / sizeof(luaT_typenames[0]) == LUA_T_COUNT, "luaT_typenames size mismatch");
static_assert(sizeof(luaT_eventname) / sizeof(luaT_eventname[0]) == TM_N, "luaT_eventname size mismatch");
static_assert(TM_EQ < 8, "fasttm optimization stores a bitfield with metamethods in a byte");

void luaT_init(lua_State* L)
{
    int i;
    for (i = 0; i < LUA_T_COUNT; i++)
    {
        L->global->ttname[i] = luaS_new(L, luaT_typenames[i]);
        luaS_fix(L->global->ttname[i]);
    }
    for (i = 0; i < TM_N; i++)
    {
        L->global->tmname[i] = luaS_new(L, luaT_eventname[i]);
        luaS_fix(L->global->tmname[i]);
    }
}

const TValue* luaT_gettm(LuaTable* events, TMS event, TString* ename)
{
    const TValue* tm = luaH_getstr(events, ename);
    LUAU_ASSERT(event <= TM_EQ);
    if (ttisnil(tm))
    {
        events->tmcache |= cast_byte(1u << event);
        return NULL;
    }
    else
        return tm;
}

const TValue* luaT_gettmbyobj(lua_State* L, const TValue* o, TMS event)
{
    LuaTable* mt;
    switch (ttype(o))
    {
    case LUA_TTABLE:
        mt = hvalue(o)->metatable;
        break;
    case LUA_TUSERDATA:
        mt = uvalue(o)->metatable;
        break;
    default:
        mt = L->global->mt[ttype(o)];
    }
    return (mt ? luaH_getstr(mt, L->global->tmname[event]) : luaO_nilobject);
}

const TString* luaT_objtypenamestr(lua_State* L, const TValue* o)
{
    if (ttisuserdata(o) && uvalue(o)->tag != UTAG_PROXY && uvalue(o)->metatable)
    {
        const TValue* type = luaH_getstr(uvalue(o)->metatable, L->global->tmname[TM_TYPE]);

        if (ttisstring(type))
            return tsvalue(type);

        return L->global->ttname[ttype(o)];
    }

    if (ttislightuserdata(o))
    {
        int tag = lightuserdatatag(o);

        if (unsigned(tag) < LUA_LUTAG_LIMIT)
        {
            if (const TString* name = L->global->lightuserdataname[tag])
                return name;
        }
    }

    if (LuaTable* mt = L->global->mt[ttype(o)])
    {
        const TValue* type = luaH_getstr(mt, L->global->tmname[TM_TYPE]);

        if (ttisstring(type))
            return tsvalue(type);
    }

    return L->global->ttname[ttype(o)];
}

const char* luaT_objtypename(lua_State* L, const TValue* o)
{
    return getstr(luaT_objtypenamestr(L, o));
}
