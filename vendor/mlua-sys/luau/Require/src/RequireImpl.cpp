#include "RequireImpl.h"

#include "Navigation.h"

#include "Luau/RequireNavigator.h"
#include "Luau/Require.h"

#include "lua.h"
#include "lualib.h"

namespace Luau::Require
{

static const char* registeredCacheTableKey = "_REGISTEREDMODULES";

static const char* requiredCacheTableKey = "_MODULES";

struct ResolvedRequire
{
    static ResolvedRequire fromErrorHandler(const RuntimeErrorHandler& errorHandler)
    {
        return {ResolvedRequire::Status::ErrorReported, "", "", "", errorHandler.getReportedError()};
    }

    static ResolvedRequire fromErrorMessage(const char* message)
    {
        return {ResolvedRequire::Status::ErrorReported, "", "", "", message};
    }

    enum class Status
    {
        Cached,
        ModuleRead,
        ErrorReported
    };

    Status status;
    std::string chunkname;
    std::string loadname;
    std::string cacheKey;
    std::string error;
};

static bool isCached(lua_State* L, const std::string& key)
{
    luaL_findtable(L, LUA_REGISTRYINDEX, requiredCacheTableKey, 1);
    lua_getfield(L, -1, key.c_str());
    bool cached = !lua_isnil(L, -1);
    lua_pop(L, 2);

    return cached;
}

static ResolvedRequire resolveRequire(luarequire_Configuration* lrc, lua_State* L, void* ctx, const char* requirerChunkname, std::string path)
{
    if (!lrc->is_require_allowed(L, ctx, requirerChunkname))
        return ResolvedRequire::fromErrorMessage("require is not supported in this context");

    RuntimeNavigationContext navigationContext{lrc, L, ctx, requirerChunkname};
    RuntimeErrorHandler errorHandler{path};

    Navigator navigator(navigationContext, errorHandler);

    Navigator::Status status = navigator.navigate(std::move(path));
    if (status == Navigator::Status::ErrorReported)
        return ResolvedRequire::fromErrorHandler(errorHandler);

    if (!navigationContext.isModulePresent())
        return ResolvedRequire::fromErrorMessage("no module present at resolved path");

    std::optional<std::string> cacheKey = navigationContext.getCacheKey();
    if (!cacheKey)
        return ResolvedRequire::fromErrorMessage("could not get cache key for module");

    if (isCached(L, *cacheKey))
    {
        lua_getfield(L, LUA_REGISTRYINDEX, requiredCacheTableKey);
        lua_getfield(L, -1, cacheKey->c_str());
        lua_remove(L, -2);

        return ResolvedRequire{ResolvedRequire::Status::Cached};
    }

    std::optional<std::string> chunkname = navigationContext.getChunkname();
    if (!chunkname)
        return ResolvedRequire::fromErrorMessage("could not get chunkname for module");

    std::optional<std::string> loadname = navigationContext.getLoadname();
    if (!loadname)
        return ResolvedRequire::fromErrorMessage("could not get loadname for module");

    return ResolvedRequire{
        ResolvedRequire::Status::ModuleRead,
        std::move(*chunkname),
        std::move(*loadname),
        std::move(*cacheKey),
    };
}

static int checkRegisteredModules(lua_State* L, const char* path)
{
    luaL_findtable(L, LUA_REGISTRYINDEX, registeredCacheTableKey, 1);

    std::string pathLower = std::string(path);
    for (char& c : pathLower)
    {
        if (c >= 'A' && c <= 'Z')
            c -= ('A' - 'a');
    }

    lua_getfield(L, -1, pathLower.c_str());
    if (lua_isnil(L, -1))
    {
        lua_pop(L, 2);
        return 0;
    }

    lua_remove(L, -2);
    return 1;
}

static const int kRequireStackValues = 4;

int lua_requirecont(lua_State* L, int status)
{
    LUAU_ASSERT(lua_gettop(L) >= kRequireStackValues);
    const int numResults = lua_gettop(L) - kRequireStackValues;
    const char* cacheKey = luaL_checkstring(L, 2);

    if (numResults > 1)
        luaL_error(L, "module must return a single value");

    if (numResults == 1)
    {

        lua_getfield(L, LUA_REGISTRYINDEX, requiredCacheTableKey);

        lua_pushvalue(L, -2);

        lua_setfield(L, -2, cacheKey);

        lua_pop(L, 1);
    }

    return numResults;
}

int lua_requireinternal(lua_State* L, const char* requirerChunkname)
{
    lua_settop(L, 1);

    luarequire_Configuration* lrc = static_cast<luarequire_Configuration*>(lua_touserdata(L, lua_upvalueindex(1)));
    if (!lrc)
        luaL_error(L, "unable to find require configuration");

    void* ctx = lua_tolightuserdata(L, lua_upvalueindex(2));

    const char* path = luaL_checkstring(L, 1);

    if (checkRegisteredModules(L, path) == 1)
        return 1;

    bool resolveError = false;

    {
        ResolvedRequire resolvedRequire = resolveRequire(lrc, L, ctx, requirerChunkname, path);

        if (resolvedRequire.status == ResolvedRequire::Status::Cached)
            return 1;

        if (resolvedRequire.status == ResolvedRequire::Status::ErrorReported)
        {
            lua_pushstring(L, resolvedRequire.error.c_str());
            resolveError = true;
        }
        else
        {
            lua_pushstring(L, resolvedRequire.cacheKey.c_str());
            lua_pushstring(L, resolvedRequire.chunkname.c_str());
            lua_pushstring(L, resolvedRequire.loadname.c_str());
        }
    }

    if (resolveError)
        lua_error(L);

    int stackValues = lua_gettop(L);
    LUAU_ASSERT(stackValues == kRequireStackValues);

    const char* chunkname = lua_tostring(L, -2);
    const char* loadname = lua_tostring(L, -1);

    int numResults = lrc->load(L, ctx, path, chunkname, loadname);
    if (numResults == -1)
    {
        if (lua_gettop(L) != stackValues)
            luaL_error(L, "stack cannot be modified when require yields");

        return lua_yield(L, 0);
    }

    return lua_requirecont(L, LUA_OK);
}

int lua_proxyrequire(lua_State* L)
{
    const char* requirerChunkname = luaL_checkstring(L, 2);
    return lua_requireinternal(L, requirerChunkname);
}

int lua_require(lua_State* L)
{
    lua_Debug ar;
    int level = 1;

    do
    {
        if (!lua_getinfo(L, level++, "s", &ar))
            luaL_error(L, "require is not supported in this context");
    } while (ar.what[0] == 'C');

    return lua_requireinternal(L, ar.source);
}

int registerModuleImpl(lua_State* L)
{
    if (lua_gettop(L) != 2)
        luaL_error(L, "expected 2 arguments: aliased require path and desired result");

    size_t len;
    const char* path = luaL_checklstring(L, 1, &len);
    std::string_view pathView(path, len);
    if (pathView.empty() || pathView[0] != '@')
        luaL_argerrorL(L, 1, "path must begin with '@'");

    std::string pathLower = std::string(path, len);
    for (char& c : pathLower)
    {
        if (c >= 'A' && c <= 'Z')
            c -= ('A' - 'a');
    }
    lua_pushstring(L, pathLower.c_str());
    lua_replace(L, 1);

    luaL_findtable(L, LUA_REGISTRYINDEX, registeredCacheTableKey, 1);

    lua_insert(L, 1);

    lua_settable(L, 1);

    lua_pop(L, 1);

    return 0;
}

int clearCacheEntry(lua_State* L)
{
    const char* cacheKey = luaL_checkstring(L, 1);
    luaL_findtable(L, LUA_REGISTRYINDEX, requiredCacheTableKey, 1);
    lua_pushnil(L);
    lua_setfield(L, -2, cacheKey);
    lua_pop(L, 1);
    return 0;
}

int clearCache(lua_State* L)
{
    lua_newtable(L);
    lua_setfield(L, LUA_REGISTRYINDEX, requiredCacheTableKey);
    return 0;
}

}
