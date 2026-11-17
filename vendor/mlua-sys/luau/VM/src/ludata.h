#pragma once

#include "lobject.h"

#define UTAG_IDTOR LUA_UTAG_LIMIT

#define UTAG_PROXY (LUA_UTAG_LIMIT + 1)

#define sizeudata(len) (offsetof(Udata, data) + (len > 16 ? ((len + 15) & ~15) : len))

LUAI_FUNC Udata* luaU_newudata(lua_State* L, size_t s, int tag);
LUAI_FUNC void luaU_freeudata(lua_State* L, Udata* u, struct lua_Page* page);
