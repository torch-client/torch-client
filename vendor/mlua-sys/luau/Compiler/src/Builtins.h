#pragma once

#include "ValueTracking.h"

namespace Luau
{
struct CompileOptions;
}

namespace Luau
{
namespace Compile
{

struct Builtin
{
    AstName object;
    AstName method;

    bool empty() const
    {
        return object == AstName() && method == AstName();
    }

    bool isGlobal(const char* name) const
    {
        return object == AstName() && method == name;
    }

    bool isMethod(const char* table, const char* name) const
    {
        return object == table && method == name;
    }
};

Builtin getBuiltin(AstExpr* node, const DenseHashMap<AstName, Global>& globals, const DenseHashMap<AstLocal*, Variable>& variables);

void analyzeBuiltins(
    DenseHashMap<AstExprCall*, int>& result,
    const DenseHashMap<AstName, Global>& globals,
    const DenseHashMap<AstLocal*, Variable>& variables,
    const CompileOptions& options,
    AstNode* root,
    const AstNameTable& names
);

struct BuiltinInfo
{
    enum Flags
    {
        Flag_NoneSafe = 1 << 0,
    };

    int params;
    int results;
    unsigned int flags;
};

BuiltinInfo getBuiltinInfo(int bfid);

}
}
