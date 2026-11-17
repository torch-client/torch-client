#include "Luau/LinterConfig.h"

#include "Luau/ParseResult.h"

namespace Luau
{

void LintOptions::setDefaults()
{
    warningMask = ~0ull;
}

const char* LintWarning::getName(Code code)
{
    LUAU_ASSERT(unsigned(code) < Code__Count);

    return kWarningNames[code];
}

LintWarning::Code LintWarning::parseName(const char* name)
{
    for (int code = Code_Unknown; code < Code__Count; ++code)
        if (strcmp(name, getName(Code(code))) == 0)
            return Code(code);

    return Code_Unknown;
}

uint64_t LintWarning::parseMask(const std::vector<HotComment>& hotcomments)
{
    uint64_t result = 0;

    for (const HotComment& hc : hotcomments)
    {
        if (!hc.header)
            continue;

        if (hc.content.compare(0, 6, "nolint") != 0)
            continue;

        size_t name = hc.content.find_first_not_of(" \t", 6);

        if (name == std::string::npos)
            return ~0ull;

        if (name == 6)
            continue;

        LintWarning::Code code = LintWarning::parseName(hc.content.c_str() + name);

        if (code != LintWarning::Code_Unknown)
            result |= 1ull << int(code);
    }

    return result;
}

}
