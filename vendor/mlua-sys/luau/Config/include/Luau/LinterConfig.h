#pragma once

#include "Luau/Location.h"

#include <string>
#include <vector>

#include <stdint.h>

namespace Luau
{

struct HotComment;

struct LintWarning
{
    enum Code
    {
        Code_Unknown = 0,

        Code_UnknownGlobal = 1,
        Code_DeprecatedGlobal = 2,
        Code_GlobalUsedAsLocal = 3,
        Code_LocalShadow = 4,
        Code_SameLineStatement = 5,
        Code_MultiLineStatement = 6,
        Code_LocalUnused = 7,
        Code_FunctionUnused = 8,
        Code_ImportUnused = 9,
        Code_BuiltinGlobalWrite = 10,
        Code_PlaceholderRead = 11,
        Code_UnreachableCode = 12,
        Code_UnknownType = 13,
        Code_ForRange = 14,
        Code_UnbalancedAssignment = 15,
        Code_ImplicitReturn = 16,
        Code_DuplicateLocal = 17,
        Code_FormatString = 18,
        Code_TableLiteral = 19,
        Code_UninitializedLocal = 20,
        Code_DuplicateFunction = 21,
        Code_DeprecatedApi = 22,
        Code_TableOperations = 23,
        Code_DuplicateCondition = 24,
        Code_MisleadingAndOr = 25,
        Code_CommentDirective = 26,
        Code_IntegerParsing = 27,
        Code_ComparisonPrecedence = 28,
        Code_RedundantNativeAttribute = 29,

        Code__Count
    };

    Code code;
    Location location;
    std::string text;

    static const char* getName(Code code);
    static Code parseName(const char* name);
    static uint64_t parseMask(const std::vector<HotComment>& hotcomments);
};

struct LintOptions
{
    uint64_t warningMask = 0;

    void enableWarning(LintWarning::Code code)
    {
        warningMask |= 1ull << code;
    }
    void disableWarning(LintWarning::Code code)
    {
        warningMask &= ~(1ull << code);
    }

    bool isEnabled(LintWarning::Code code) const
    {
        return 0 != (warningMask & (1ull << code));
    }

    void setDefaults();
};

inline constexpr const char* kWarningNames[] = {
    "Unknown",

    "UnknownGlobal",
    "DeprecatedGlobal",
    "GlobalUsedAsLocal",
    "LocalShadow",
    "SameLineStatement",
    "MultiLineStatement",
    "LocalUnused",
    "FunctionUnused",
    "ImportUnused",
    "BuiltinGlobalWrite",
    "PlaceholderRead",
    "UnreachableCode",
    "UnknownType",
    "ForRange",
    "UnbalancedAssignment",
    "ImplicitReturn",
    "DuplicateLocal",
    "FormatString",
    "TableLiteral",
    "UninitializedLocal",
    "DuplicateFunction",
    "DeprecatedApi",
    "TableOperations",
    "DuplicateCondition",
    "MisleadingAndOr",
    "CommentDirective",
    "IntegerParsing",
    "ComparisonPrecedence",
    "RedundantNativeAttribute",
};

static_assert(std::size(kWarningNames) == unsigned(LintWarning::Code__Count), "did you forget to add warning to the list?");

}
