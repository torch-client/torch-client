#pragma once

#include "Luau/Ast.h"
#include "Luau/DenseHash.h"

namespace Luau
{
namespace Compile
{

struct TableShape
{
    unsigned int arraySize = 0;
    unsigned int hashSize = 0;
};

void predictTableShapes(DenseHashMap<AstExprTable*, TableShape>& shapes, AstNode* root);

}
}
