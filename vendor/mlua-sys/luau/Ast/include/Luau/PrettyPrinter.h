#pragma once

#include "Luau/Location.h"
#include "Luau/ParseOptions.h"
#include "Luau/ParseResult.h"

#include <string>

namespace Luau
{
class AstNode;
class AstStatBlock;

struct PrettyPrintResult
{
    std::string code;
    Location errorLocation;
    std::string parseError;
};

std::string toString(AstNode* node);
void dump(AstNode* node);

std::string prettyPrint(AstStatBlock& ast);
std::string prettyPrintWithTypes(AstStatBlock& block);
std::string prettyPrintWithTypes(AstStatBlock& block, const CstNodeMap& cstNodeMap);

PrettyPrintResult prettyPrint(std::string_view source, ParseOptions options = ParseOptions{}, bool withTypes = false);

}
