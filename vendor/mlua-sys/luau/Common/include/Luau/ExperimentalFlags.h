#pragma once

#include <string.h>

namespace Luau
{

inline bool isAnalysisFlagExperimental(const char* flag)
{
    static const char* const kList[] = {
        "LuauInstantiateInSubtyping",
        "LuauFixIndexerSubtypingOrdering",
        "LuauSolverV2",
        "UseNewLuauTypeSolverDefaultEnabled",
        nullptr,
    };

    for (const char* item : kList)
        if (item && strcmp(item, flag) == 0)
            return true;

    return false;
}

}
