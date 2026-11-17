#pragma once

#include <stddef.h>

#ifndef LUACODE_API
#define LUACODE_API extern
#endif

typedef struct lua_CompileOptions lua_CompileOptions;
typedef void* lua_CompileConstant;

typedef int (*lua_LibraryMemberTypeCallback)(const char* library, const char* member);

typedef void (*lua_LibraryMemberConstantCallback)(const char* library, const char* member, lua_CompileConstant* constant);

struct lua_CompileOptions
{
    int optimizationLevel;

    int debugLevel;

    int typeInfoLevel;

    int coverageLevel;

    const char* vectorLib;
    const char* vectorCtor;

    const char* vectorType;

    const char* const* mutableGlobals;

    const char* const* userdataTypes;

    const char* const* librariesWithKnownMembers;
    lua_LibraryMemberTypeCallback libraryMemberTypeCb;
    lua_LibraryMemberConstantCallback libraryMemberConstantCb;

    const char* const* disabledBuiltins;
};

LUACODE_API char* luau_compile(const char* source, size_t size, lua_CompileOptions* options, size_t* outsize);

LUACODE_API void luau_set_compile_constant_nil(lua_CompileConstant* constant);
LUACODE_API void luau_set_compile_constant_boolean(lua_CompileConstant* constant, int b);
LUACODE_API void luau_set_compile_constant_number(lua_CompileConstant* constant, double n);
LUACODE_API void luau_set_compile_constant_vector(lua_CompileConstant* constant, float x, float y, float z, float w);
LUACODE_API void luau_set_compile_constant_string(lua_CompileConstant* constant, const char* s, size_t l);
