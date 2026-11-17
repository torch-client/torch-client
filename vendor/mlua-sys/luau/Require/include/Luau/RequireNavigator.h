#pragma once

#include "Luau/Config.h"
#include "Luau/LuauConfig.h"

#include <functional>
#include <optional>
#include <string>
#include <string_view>

struct lua_State;

namespace Luau::Require
{

class AliasCycleTracker;

class ErrorHandler
{
public:
    virtual ~ErrorHandler() = default;
    virtual void reportError(std::string message) {}
};

class NavigationContext
{
public:
    virtual ~NavigationContext() = default;

    enum class NavigateResult
    {
        Success,
        Ambiguous,
        NotFound
    };

    virtual NavigateResult resetToRequirer() = 0;
    virtual NavigateResult jumpToAlias(const std::string& path) = 0;

    virtual NavigateResult toAliasOverride(const std::string& aliasUnprefixed)
    {
        return NavigateResult::NotFound;
    };
    virtual NavigateResult toAliasFallback(const std::string& aliasUnprefixed)
    {
        return NavigateResult::NotFound;
    };

    virtual NavigateResult toParent() = 0;
    virtual NavigateResult toChild(const std::string& component) = 0;

    enum class ConfigBehavior
    {
        GetAlias,
        GetConfig
    };

    enum class ConfigStatus
    {
        Absent,
        Ambiguous,
        PresentJson,
        PresentLuau
    };

    virtual ConfigStatus getConfigStatus() const = 0;

    std::function<void(lua_State*)> luauConfigInit = nullptr;
    void (*luauConfigInterrupt)(lua_State* L, int gc) = nullptr;

    virtual ConfigBehavior getConfigBehavior() const = 0;
    virtual std::optional<std::string> getAlias(const std::string& alias) const = 0;
    virtual std::optional<std::string> getConfig() const = 0;
};

class Navigator
{
public:
    enum class Status
    {
        Success,
        ErrorReported
    };

    Navigator(NavigationContext& navigationContext, ErrorHandler& errorHandler);
    [[nodiscard]] Status navigate(std::string path);

private:
    using Error = std::optional<std::string>;
    [[nodiscard]] Error navigateImpl(std::string_view path);
    [[nodiscard]] Error navigateThroughPath(std::string_view path);
    [[nodiscard]] Error navigateToAlias(const std::string& alias, const Config& config, AliasCycleTracker cycleTracker);
    [[nodiscard]] Error navigateToAndPopulateConfig(const std::string& desiredAlias, Config& config);

    [[nodiscard]] Error resetToRequirer();
    [[nodiscard]] Error jumpToAlias(const std::string& aliasPath);
    [[nodiscard]] Error navigateToParent(std::optional<std::string> previousComponent);
    [[nodiscard]] Error navigateToChild(const std::string& component);

    [[nodiscard]] std::pair<Error, bool> toAliasOverride(const std::string& aliasUnprefixed);
    [[nodiscard]] Error toAliasFallback(const std::string& aliasUnprefixed);

    NavigationContext& navigationContext;
    ErrorHandler& errorHandler;
};

}
