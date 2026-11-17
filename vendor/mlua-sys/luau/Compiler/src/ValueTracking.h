#pragma once

#include "Luau/Ast.h"
#include "Luau/DenseHash.h"

namespace Luau
{
class AstNameTable;
}

namespace Luau
{
namespace Compile
{

enum class Global
{
    Default = 0,
    Mutable,
    Written,
};

struct Variable
{
    AstExpr* init = nullptr;
    bool written = false;
    bool constant = false;
};

void assignMutable(DenseHashMap<AstName, Global>& globals, const AstNameTable& names, const char* const* mutableGlobals);
void trackValues(DenseHashMap<AstName, Global>& globals, DenseHashMap<AstLocal*, Variable>& variables, AstNode* root);

inline Global getGlobalState(const DenseHashMap<AstName, Global>& globals, AstName name)
{
    const Global* it = globals.find(name);

    return it ? *it : Global::Default;
}

}
}
