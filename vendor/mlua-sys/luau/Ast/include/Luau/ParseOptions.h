#pragma once

#include "Luau/Ast.h"
#include "Luau/DenseHash.h"

#include <vector>

namespace Luau
{

enum class Mode
{
    NoCheck,
    Nonstrict,
    Strict,
    Definition,
};

struct FragmentParseResumeSettings
{
    DenseHashMap<AstName, AstLocal*> localMap{AstName()};
    std::vector<AstLocal*> localStack;
    Position resumePosition;
};

struct ParseOptions
{
    bool allowDeclarationSyntax = false;
    bool captureComments = false;
    std::optional<FragmentParseResumeSettings> parseFragment = std::nullopt;
    bool storeCstData = false;
    bool noErrorLimit = false;
};

}
