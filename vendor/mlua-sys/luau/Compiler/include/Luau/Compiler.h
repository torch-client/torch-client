#pragma once

#include "Luau/ParseOptions.h"
#include "Luau/Location.h"
#include "Luau/StringUtils.h"
#include "Luau/Common.h"

namespace Luau
{
class AstNameTable;
struct ParseResult;
class BytecodeBuilder;
class BytecodeEncoder;

using CompileConstant = void*;

using LibraryMemberTypeCallback = int (*)(const char* library, const char* member);

using LibraryMemberConstantCallback = void (*)(const char* library, const char* member, CompileConstant* constant);

struct CompileOptions
{
    int optimizationLevel = 1;

    int debugLevel = 1;

    int typeInfoLevel = 0;

    int coverageLevel = 0;

    const char* vectorLib = nullptr;
    const char* vectorCtor = nullptr;

    const char* vectorType = nullptr;

    const char* const* mutableGlobals = nullptr;

    const char* const* userdataTypes = nullptr;

    const char* const* librariesWithKnownMembers = nullptr;
    LibraryMemberTypeCallback libraryMemberTypeCb = nullptr;
    LibraryMemberConstantCallback libraryMemberConstantCb = nullptr;

    const char* const* disabledBuiltins = nullptr;
};

class CompileError : public std::exception
{
public:
    CompileError(const Location& location, std::string message);

    ~CompileError() throw() override;

    const char* what() const throw() override;

    const Location& getLocation() const;

    static LUAU_NORETURN void raise(const Location& location, const char* format, ...) LUAU_PRINTF_ATTR(2, 3);

private:
    Location location;
    std::string message;
};

void compileOrThrow(BytecodeBuilder& bytecode, const ParseResult& parseResult, AstNameTable& names, const CompileOptions& options = {});
void compileOrThrow(BytecodeBuilder& bytecode, const std::string& source, const CompileOptions& options = {}, const ParseOptions& parseOptions = {});

std::string compile(
    const std::string& source,
    const CompileOptions& options = {},
    const ParseOptions& parseOptions = {},
    BytecodeEncoder* encoder = nullptr
);

void setCompileConstantNil(CompileConstant* constant);
void setCompileConstantBoolean(CompileConstant* constant, bool b);
void setCompileConstantNumber(CompileConstant* constant, double n);
void setCompileConstantVector(CompileConstant* constant, float x, float y, float z, float w);
void setCompileConstantString(CompileConstant* constant, const char* s, size_t l);

}
