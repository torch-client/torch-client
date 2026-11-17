#pragma once

#include "ConstantFolding.h"

namespace Luau
{

class AstNameTable;

namespace Compile
{

Constant foldBuiltin(AstNameTable& stringTable, int bfid, const Constant* args, size_t count);
Constant foldBuiltinMath(AstName index);

}
}
