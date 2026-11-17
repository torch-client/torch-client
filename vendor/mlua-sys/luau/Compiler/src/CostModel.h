#pragma once

#include "Luau/Ast.h"
#include "Luau/DenseHash.h"

namespace Luau
{
namespace Compile
{

struct Constant;

uint64_t modelCost(
    AstNode* root,
    AstLocal* const* vars,
    size_t varCount,
    const DenseHashMap<AstExprCall*, int>& builtins,
    const DenseHashMap<AstExpr*, Constant>& constants
);
uint64_t modelCost(AstNode* root, AstLocal* const* vars, size_t varCount);

int computeCost(uint64_t model, const bool* varsConst, size_t varCount);

int getTripCount(double from, double to, double step);

}
}
