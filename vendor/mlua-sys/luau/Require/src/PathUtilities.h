#pragma once

#include <string_view>
#include <utility>

namespace Luau::Require
{

enum class PathType
{
    RelativeToCurrent,
    RelativeToParent,
    Aliased,
    Unsupported
};

PathType getPathType(std::string_view path);

std::pair<std::string_view, std::string_view> splitPath(std::string_view path);

}
