#pragma once

#include "lobject.h"

#define MAX_BUFFER_SIZE (1 << 30)

#define sizebuffer(len) (offsetof(Buffer, data) + ((len) < 8 ? 8 : (len)))

LUAI_FUNC Buffer* luaB_newbuffer(lua_State* L, size_t s);
LUAI_FUNC void luaB_freebuffer(lua_State* L, Buffer* u, struct lua_Page* page);
