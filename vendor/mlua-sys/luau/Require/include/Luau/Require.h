#pragma once

#include "lua.h"

#include <stdbool.h>
#include <stddef.h>

typedef enum luarequire_NavigateResult
{
    NAVIGATE_SUCCESS,
    NAVIGATE_AMBIGUOUS,
    NAVIGATE_NOT_FOUND
} luarequire_NavigateResult;

typedef enum luarequire_WriteResult
{
    WRITE_SUCCESS,
    WRITE_BUFFER_TOO_SMALL,
    WRITE_FAILURE
} luarequire_WriteResult;

typedef enum luarequire_ConfigStatus
{
    CONFIG_ABSENT,
    CONFIG_AMBIGUOUS,
    CONFIG_PRESENT_JSON,
    CONFIG_PRESENT_LUAU,
} luarequire_ConfigStatus;

typedef struct luarequire_Configuration
{
    bool (*is_require_allowed)(lua_State* L, void* ctx, const char* requirer_chunkname);

    luarequire_NavigateResult (*reset)(lua_State* L, void* ctx, const char* requirer_chunkname);

    luarequire_NavigateResult (*jump_to_alias)(lua_State* L, void* ctx, const char* path);

    luarequire_NavigateResult (*to_alias_override)(lua_State* L, void* ctx, const char* alias_unprefixed);

    luarequire_NavigateResult (*to_alias_fallback)(lua_State* L, void* ctx, const char* alias_unprefixed);

    luarequire_NavigateResult (*to_parent)(lua_State* L, void* ctx);
    luarequire_NavigateResult (*to_child)(lua_State* L, void* ctx, const char* name);

    bool (*is_module_present)(lua_State* L, void* ctx);

    luarequire_WriteResult (*get_chunkname)(lua_State* L, void* ctx, char* buffer, size_t buffer_size, size_t* size_out);

    luarequire_WriteResult (*get_loadname)(lua_State* L, void* ctx, char* buffer, size_t buffer_size, size_t* size_out);

    luarequire_WriteResult (*get_cache_key)(lua_State* L, void* ctx, char* buffer, size_t buffer_size, size_t* size_out);

    luarequire_ConfigStatus (*get_config_status)(lua_State* L, void* ctx);

    luarequire_WriteResult (*get_alias)(lua_State* L, void* ctx, const char* alias, char* buffer, size_t buffer_size, size_t* size_out);

    luarequire_WriteResult (*get_config)(lua_State* L, void* ctx, char* buffer, size_t buffer_size, size_t* size_out);

    int (*get_luau_config_timeout)(lua_State* L, void* ctx);

    int (*load)(lua_State* L, void* ctx, const char* path, const char* chunkname, const char* loadname);
} luarequire_Configuration;

typedef void (*luarequire_Configuration_init)(luarequire_Configuration* config);

LUALIB_API int luarequire_pushrequire(lua_State* L, luarequire_Configuration_init config_init, void* ctx);

LUALIB_API void luaopen_require(lua_State* L, luarequire_Configuration_init config_init, void* ctx);

LUALIB_API int luarequire_pushproxyrequire(lua_State* L, luarequire_Configuration_init config_init, void* ctx);

LUALIB_API int luarequire_registermodule(lua_State* L);

LUALIB_API int luarequire_clearcacheentry(lua_State* L);

LUALIB_API int luarequire_clearcache(lua_State* L);
