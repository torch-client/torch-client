#include "ConstantFolding.h"

#include "BuiltinFolding.h"
#include "Luau/Bytecode.h"
#include "Luau/Lexer.h"

#include <vector>
#include <math.h>

LUAU_FASTFLAG(LuauExplicitTypeInstantiationSyntax)
LUAU_FASTFLAGVARIABLE(LuauCompileFoldVectorComp)
LUAU_FASTFLAG(LuauCompileInlinedBuiltins)

namespace Luau
{
namespace Compile
{

static bool constantsEqual(const Constant& la, const Constant& ra)
{
    LUAU_ASSERT(la.type != Constant::Type_Unknown && ra.type != Constant::Type_Unknown);

    switch (la.type)
    {
    case Constant::Type_Nil:
        return ra.type == Constant::Type_Nil;

    case Constant::Type_Boolean:
        return ra.type == Constant::Type_Boolean && la.valueBoolean == ra.valueBoolean;

    case Constant::Type_Number:
        return ra.type == Constant::Type_Number && la.valueNumber == ra.valueNumber;

    case Constant::Type_Vector:
        return ra.type == Constant::Type_Vector && la.valueVector[0] == ra.valueVector[0] && la.valueVector[1] == ra.valueVector[1] &&
               la.valueVector[2] == ra.valueVector[2] && la.valueVector[3] == ra.valueVector[3];

    case Constant::Type_String:
        return ra.type == Constant::Type_String && la.stringLength == ra.stringLength && memcmp(la.valueString, ra.valueString, la.stringLength) == 0;

    default:
        LUAU_ASSERT(!"Unexpected constant type in comparison");
        return false;
    }
}

static void foldUnary(Constant& result, AstExprUnary::Op op, const Constant& arg)
{
    switch (op)
    {
    case AstExprUnary::Not:
        if (arg.type != Constant::Type_Unknown)
        {
            result.type = Constant::Type_Boolean;
            result.valueBoolean = !arg.isTruthful();
        }
        break;

    case AstExprUnary::Minus:
        if (arg.type == Constant::Type_Number)
        {
            result.type = Constant::Type_Number;
            result.valueNumber = -arg.valueNumber;
        }
        else if (arg.type == Constant::Type_Vector)
        {
            result.type = Constant::Type_Vector;
            result.valueVector[0] = -arg.valueVector[0];
            result.valueVector[1] = -arg.valueVector[1];
            result.valueVector[2] = -arg.valueVector[2];
            result.valueVector[3] = -arg.valueVector[3];
        }
        break;

    case AstExprUnary::Len:
        if (arg.type == Constant::Type_String)
        {
            result.type = Constant::Type_Number;
            result.valueNumber = double(arg.stringLength);
        }
        break;

    default:
        LUAU_ASSERT(!"Unexpected unary operation");
    }
}

static void foldBinary(Constant& result, AstExprBinary::Op op, const Constant& la, const Constant& ra, AstNameTable& stringTable)
{
    switch (op)
    {
    case AstExprBinary::Add:
        if (la.type == Constant::Type_Number && ra.type == Constant::Type_Number)
        {
            result.type = Constant::Type_Number;
            result.valueNumber = la.valueNumber + ra.valueNumber;
        }
        else if (la.type == Constant::Type_Vector && ra.type == Constant::Type_Vector)
        {
            result.type = Constant::Type_Vector;
            result.valueVector[0] = la.valueVector[0] + ra.valueVector[0];
            result.valueVector[1] = la.valueVector[1] + ra.valueVector[1];
            result.valueVector[2] = la.valueVector[2] + ra.valueVector[2];
            result.valueVector[3] = la.valueVector[3] + ra.valueVector[3];
        }
        break;

    case AstExprBinary::Sub:
        if (la.type == Constant::Type_Number && ra.type == Constant::Type_Number)
        {
            result.type = Constant::Type_Number;
            result.valueNumber = la.valueNumber - ra.valueNumber;
        }
        else if (la.type == Constant::Type_Vector && ra.type == Constant::Type_Vector)
        {
            result.type = Constant::Type_Vector;
            result.valueVector[0] = la.valueVector[0] - ra.valueVector[0];
            result.valueVector[1] = la.valueVector[1] - ra.valueVector[1];
            result.valueVector[2] = la.valueVector[2] - ra.valueVector[2];
            result.valueVector[3] = la.valueVector[3] - ra.valueVector[3];
        }
        break;

    case AstExprBinary::Mul:
        if (la.type == Constant::Type_Number && ra.type == Constant::Type_Number)
        {
            result.type = Constant::Type_Number;
            result.valueNumber = la.valueNumber * ra.valueNumber;
        }
        else if (la.type == Constant::Type_Vector && ra.type == Constant::Type_Vector)
        {
            bool hadW = la.valueVector[3] != 0.0f || ra.valueVector[3] != 0.0f;
            float resultW = la.valueVector[3] * ra.valueVector[3];

            if (resultW == 0.0f || hadW)
            {
                result.type = Constant::Type_Vector;
                result.valueVector[0] = la.valueVector[0] * ra.valueVector[0];
                result.valueVector[1] = la.valueVector[1] * ra.valueVector[1];
                result.valueVector[2] = la.valueVector[2] * ra.valueVector[2];
                result.valueVector[3] = resultW;
            }
        }
        else if (la.type == Constant::Type_Number && ra.type == Constant::Type_Vector)
        {
            bool hadW = ra.valueVector[3] != 0.0f;
            float resultW = float(la.valueNumber) * ra.valueVector[3];

            if (resultW == 0.0f || hadW)
            {
                result.type = Constant::Type_Vector;
                result.valueVector[0] = float(la.valueNumber) * ra.valueVector[0];
                result.valueVector[1] = float(la.valueNumber) * ra.valueVector[1];
                result.valueVector[2] = float(la.valueNumber) * ra.valueVector[2];
                result.valueVector[3] = resultW;
            }
        }
        else if (la.type == Constant::Type_Vector && ra.type == Constant::Type_Number)
        {
            bool hadW = la.valueVector[3] != 0.0f;
            float resultW = la.valueVector[3] * float(ra.valueNumber);

            if (resultW == 0.0f || hadW)
            {
                result.type = Constant::Type_Vector;
                result.valueVector[0] = la.valueVector[0] * float(ra.valueNumber);
                result.valueVector[1] = la.valueVector[1] * float(ra.valueNumber);
                result.valueVector[2] = la.valueVector[2] * float(ra.valueNumber);
                result.valueVector[3] = resultW;
            }
        }
        break;

    case AstExprBinary::Div:
        if (la.type == Constant::Type_Number && ra.type == Constant::Type_Number)
        {
            result.type = Constant::Type_Number;
            result.valueNumber = la.valueNumber / ra.valueNumber;
        }
        else if (la.type == Constant::Type_Vector && ra.type == Constant::Type_Vector)
        {
            bool hadW = la.valueVector[3] != 0.0f || ra.valueVector[3] != 0.0f;
            float resultW = la.valueVector[3] / ra.valueVector[3];

            if (resultW == 0.0f || hadW)
            {
                result.type = Constant::Type_Vector;
                result.valueVector[0] = la.valueVector[0] / ra.valueVector[0];
                result.valueVector[1] = la.valueVector[1] / ra.valueVector[1];
                result.valueVector[2] = la.valueVector[2] / ra.valueVector[2];
                result.valueVector[3] = resultW;
            }
        }
        else if (la.type == Constant::Type_Number && ra.type == Constant::Type_Vector)
        {
            bool hadW = ra.valueVector[3] != 0.0f;
            float resultW = float(la.valueNumber) / ra.valueVector[3];

            if (resultW == 0.0f || hadW)
            {
                result.type = Constant::Type_Vector;
                result.valueVector[0] = float(la.valueNumber) / ra.valueVector[0];
                result.valueVector[1] = float(la.valueNumber) / ra.valueVector[1];
                result.valueVector[2] = float(la.valueNumber) / ra.valueVector[2];
                result.valueVector[3] = resultW;
            }
        }
        else if (la.type == Constant::Type_Vector && ra.type == Constant::Type_Number)
        {
            bool hadW = la.valueVector[3] != 0.0f;
            float resultW = la.valueVector[3] / float(ra.valueNumber);

            if (resultW == 0.0f || hadW)
            {
                result.type = Constant::Type_Vector;
                result.valueVector[0] = la.valueVector[0] / float(ra.valueNumber);
                result.valueVector[1] = la.valueVector[1] / float(ra.valueNumber);
                result.valueVector[2] = la.valueVector[2] / float(ra.valueNumber);
                result.valueVector[3] = resultW;
            }
        }
        break;

    case AstExprBinary::FloorDiv:
        if (la.type == Constant::Type_Number && ra.type == Constant::Type_Number)
        {
            result.type = Constant::Type_Number;
            result.valueNumber = floor(la.valueNumber / ra.valueNumber);
        }
        else if (la.type == Constant::Type_Vector && ra.type == Constant::Type_Vector)
        {
            bool hadW = la.valueVector[3] != 0.0f || ra.valueVector[3] != 0.0f;
            float resultW = floor(la.valueVector[3] / ra.valueVector[3]);

            if (resultW == 0.0f || hadW)
            {
                result.type = Constant::Type_Vector;
                result.valueVector[0] = floor(la.valueVector[0] / ra.valueVector[0]);
                result.valueVector[1] = floor(la.valueVector[1] / ra.valueVector[1]);
                result.valueVector[2] = floor(la.valueVector[2] / ra.valueVector[2]);
                result.valueVector[3] = resultW;
            }
        }
        else if (la.type == Constant::Type_Number && ra.type == Constant::Type_Vector)
        {
            bool hadW = ra.valueVector[3] != 0.0f;
            float resultW = floor(float(la.valueNumber) / ra.valueVector[3]);

            if (resultW == 0.0f || hadW)
            {
                result.type = Constant::Type_Vector;
                result.valueVector[0] = floor(float(la.valueNumber) / ra.valueVector[0]);
                result.valueVector[1] = floor(float(la.valueNumber) / ra.valueVector[1]);
                result.valueVector[2] = floor(float(la.valueNumber) / ra.valueVector[2]);
                result.valueVector[3] = resultW;
            }
        }
        else if (la.type == Constant::Type_Vector && ra.type == Constant::Type_Number)
        {
            bool hadW = la.valueVector[3] != 0.0f;
            float resultW = floor(la.valueVector[3] / float(ra.valueNumber));

            if (resultW == 0.0f || hadW)
            {
                result.type = Constant::Type_Vector;
                result.valueVector[0] = floor(la.valueVector[0] / float(ra.valueNumber));
                result.valueVector[1] = floor(la.valueVector[1] / float(ra.valueNumber));
                result.valueVector[2] = floor(la.valueVector[2] / float(ra.valueNumber));
                result.valueVector[3] = floor(la.valueVector[3] / float(ra.valueNumber));
            }
        }
        break;

    case AstExprBinary::Mod:
        if (la.type == Constant::Type_Number && ra.type == Constant::Type_Number)
        {
            result.type = Constant::Type_Number;
            result.valueNumber = la.valueNumber - floor(la.valueNumber / ra.valueNumber) * ra.valueNumber;
        }
        break;

    case AstExprBinary::Pow:
        if (la.type == Constant::Type_Number && ra.type == Constant::Type_Number)
        {
            result.type = Constant::Type_Number;
            result.valueNumber = pow(la.valueNumber, ra.valueNumber);
        }
        break;

    case AstExprBinary::Concat:
        if (la.type == Constant::Type_String && ra.type == Constant::Type_String)
        {
            result.type = Constant::Type_String;
            result.stringLength = la.stringLength + ra.stringLength;
            if (la.stringLength == 0)
                result.valueString = ra.valueString;
            else if (ra.stringLength == 0)
                result.valueString = la.valueString;
            else
            {
                std::string tmp;
                tmp.reserve(result.stringLength + 1);
                tmp.append(la.valueString, la.stringLength);
                tmp.append(ra.valueString, ra.stringLength);
                AstName name = stringTable.getOrAdd(tmp.c_str(), result.stringLength);
                result.valueString = name.value;
            }
        }
        break;

    case AstExprBinary::CompareNe:
        if (la.type != Constant::Type_Unknown && ra.type != Constant::Type_Unknown)
        {
            result.type = Constant::Type_Boolean;
            result.valueBoolean = !constantsEqual(la, ra);
        }
        break;

    case AstExprBinary::CompareEq:
        if (la.type != Constant::Type_Unknown && ra.type != Constant::Type_Unknown)
        {
            result.type = Constant::Type_Boolean;
            result.valueBoolean = constantsEqual(la, ra);
        }
        break;

    case AstExprBinary::CompareLt:
        if (la.type == Constant::Type_Number && ra.type == Constant::Type_Number)
        {
            result.type = Constant::Type_Boolean;
            result.valueBoolean = la.valueNumber < ra.valueNumber;
        }
        break;

    case AstExprBinary::CompareLe:
        if (la.type == Constant::Type_Number && ra.type == Constant::Type_Number)
        {
            result.type = Constant::Type_Boolean;
            result.valueBoolean = la.valueNumber <= ra.valueNumber;
        }
        break;

    case AstExprBinary::CompareGt:
        if (la.type == Constant::Type_Number && ra.type == Constant::Type_Number)
        {
            result.type = Constant::Type_Boolean;
            result.valueBoolean = la.valueNumber > ra.valueNumber;
        }
        break;

    case AstExprBinary::CompareGe:
        if (la.type == Constant::Type_Number && ra.type == Constant::Type_Number)
        {
            result.type = Constant::Type_Boolean;
            result.valueBoolean = la.valueNumber >= ra.valueNumber;
        }
        break;

    case AstExprBinary::And:
        if (la.type != Constant::Type_Unknown)
        {
            result = la.isTruthful() ? ra : la;
        }
        break;

    case AstExprBinary::Or:
        if (la.type != Constant::Type_Unknown)
        {
            result = la.isTruthful() ? la : ra;
        }
        break;

    default:
        LUAU_ASSERT(!"Unexpected binary operation");
    }
}

static void foldInterpString(Constant& result, AstExprInterpString* expr, DenseHashMap<AstExpr*, Constant>& constants, AstNameTable& stringTable)
{
    LUAU_ASSERT(expr->strings.size == expr->expressions.size + 1);
    size_t resultLength = 0;
    for (size_t index = 0; index < expr->strings.size; ++index)
    {
        resultLength += expr->strings.data[index].size;
        if (index < expr->expressions.size)
        {
            const Constant* c = constants.find(expr->expressions.data[index]);
            LUAU_ASSERT(c != nullptr && c->type == Constant::Type::Type_String);
            resultLength += c->stringLength;
        }
    }
    result.type = Constant::Type_String;
    result.stringLength = resultLength;

    if (resultLength == 0)
    {
        result.valueString = "";
        return;
    }

    std::string tmp;
    tmp.reserve(resultLength);

    for (size_t index = 0; index < expr->strings.size; ++index)
    {
        AstArray<char> string = expr->strings.data[index];
        tmp.append(string.data, string.size);
        if (index < expr->expressions.size)
        {
            const Constant* c = constants.find(expr->expressions.data[index]);
            tmp.append(c->valueString, c->stringLength);
        }
    }
    result.type = Constant::Type_String;
    result.stringLength = resultLength;
    AstName name = stringTable.getOrAdd(tmp.c_str(), resultLength);
    result.valueString = name.value;
}

struct ConstantVisitor : AstVisitor
{
    DenseHashMap<AstExpr*, Constant>& constants;
    DenseHashMap<AstLocal*, Variable>& variables;
    DenseHashMap<AstLocal*, Constant>& locals;

    const DenseHashMap<AstExprCall*, int>* builtins;
    bool foldLibraryK = false;
    LibraryMemberConstantCallback libraryMemberConstantCb;
    AstNameTable& stringTable;

    bool wasEmpty = false;

    std::vector<Constant> builtinArgs;

    ConstantVisitor(
        DenseHashMap<AstExpr*, Constant>& constants,
        DenseHashMap<AstLocal*, Variable>& variables,
        DenseHashMap<AstLocal*, Constant>& locals,
        const DenseHashMap<AstExprCall*, int>* builtins,
        bool foldLibraryK,
        LibraryMemberConstantCallback libraryMemberConstantCb,
        AstNameTable& stringTable
    )
        : constants(constants)
        , variables(variables)
        , locals(locals)
        , builtins(builtins)
        , foldLibraryK(foldLibraryK)
        , libraryMemberConstantCb(libraryMemberConstantCb)
        , stringTable(stringTable)
    {
        wasEmpty = constants.empty() && locals.empty();
    }

    Constant analyze(AstExpr* node)
    {
        Constant result;
        result.type = Constant::Type_Unknown;

        if (AstExprGroup* expr = node->as<AstExprGroup>())
        {
            result = analyze(expr->expr);
        }
        else if (node->is<AstExprConstantNil>())
        {
            result.type = Constant::Type_Nil;
        }
        else if (AstExprConstantBool* expr = node->as<AstExprConstantBool>())
        {
            result.type = Constant::Type_Boolean;
            result.valueBoolean = expr->value;
        }
        else if (AstExprConstantNumber* expr = node->as<AstExprConstantNumber>())
        {
            result.type = Constant::Type_Number;
            result.valueNumber = expr->value;
        }
        else if (AstExprConstantString* expr = node->as<AstExprConstantString>())
        {
            result.type = Constant::Type_String;
            result.valueString = expr->value.data;
            result.stringLength = unsigned(expr->value.size);
        }
        else if (AstExprLocal* expr = node->as<AstExprLocal>())
        {
            const Constant* l = locals.find(expr->local);

            if (l)
                result = *l;
        }
        else if (node->is<AstExprGlobal>())
        {
        }
        else if (node->is<AstExprVarargs>())
        {
        }
        else if (AstExprCall* expr = node->as<AstExprCall>())
        {
            analyze(expr->func);

            if (FFlag::LuauCompileInlinedBuiltins)
            {
                const int* bfid = builtins ? builtins->find(expr) : nullptr;

                if (bfid && *bfid != LBF_NONE)
                {
                    size_t offset = builtinArgs.size();
                    bool canFold = true;

                    builtinArgs.reserve(offset + expr->args.size);

                    for (size_t i = 0; i < expr->args.size; ++i)
                    {
                        Constant ac = analyze(expr->args.data[i]);

                        if (ac.type == Constant::Type_Unknown)
                            canFold = false;
                        else
                            builtinArgs.push_back(ac);
                    }

                    if (canFold)
                    {
                        LUAU_ASSERT(builtinArgs.size() == offset + expr->args.size);
                        result = foldBuiltin(stringTable, *bfid, builtinArgs.data() + offset, expr->args.size);
                    }

                    builtinArgs.resize(offset);
                }
                else
                {
                    for (size_t i = 0; i < expr->args.size; ++i)
                        analyze(expr->args.data[i]);
                }
            }
            else
            {
                if (const int* bfid = builtins ? builtins->find(expr) : nullptr)
                {
                    size_t offset = builtinArgs.size();
                    bool canFold = true;

                    builtinArgs.reserve(offset + expr->args.size);

                    for (size_t i = 0; i < expr->args.size; ++i)
                    {
                        Constant ac = analyze(expr->args.data[i]);

                        if (ac.type == Constant::Type_Unknown)
                            canFold = false;
                        else
                            builtinArgs.push_back(ac);
                    }

                    if (canFold)
                    {
                        LUAU_ASSERT(builtinArgs.size() == offset + expr->args.size);
                        result = foldBuiltin(stringTable, *bfid, builtinArgs.data() + offset, expr->args.size);
                    }

                    builtinArgs.resize(offset);
                }
                else
                {
                    for (size_t i = 0; i < expr->args.size; ++i)
                        analyze(expr->args.data[i]);
                }
            }
        }
        else if (AstExprIndexName* expr = node->as<AstExprIndexName>())
        {
            if (FFlag::LuauCompileFoldVectorComp)
            {
                Constant value = analyze(expr->expr);

                if (value.type == Constant::Type_Vector)
                {
                    if (expr->index == "x" || expr->index == "X")
                    {
                        result.type = Constant::Type_Number;
                        result.valueNumber = value.valueVector[0];
                    }
                    else if (expr->index == "y" || expr->index == "Y")
                    {
                        result.type = Constant::Type_Number;
                        result.valueNumber = value.valueVector[1];
                    }
                    else if (expr->index == "z" || expr->index == "Z")
                    {
                        result.type = Constant::Type_Number;
                        result.valueNumber = value.valueVector[2];
                    }

                }
                else if (foldLibraryK)
                {
                    if (AstExprGlobal* eg = expr->expr->as<AstExprGlobal>())
                    {
                        if (eg->name == "math")
                            result = foldBuiltinMath(expr->index);

                        if (libraryMemberConstantCb && result.type == Constant::Type_Unknown)
                            libraryMemberConstantCb(eg->name.value, expr->index.value, reinterpret_cast<Luau::CompileConstant*>(&result));
                    }
                }
            }
            else
            {
                analyze(expr->expr);

                if (foldLibraryK)
                {
                    if (AstExprGlobal* eg = expr->expr->as<AstExprGlobal>())
                    {
                        if (eg->name == "math")
                            result = foldBuiltinMath(expr->index);

                        if (libraryMemberConstantCb && result.type == Constant::Type_Unknown)
                            libraryMemberConstantCb(eg->name.value, expr->index.value, reinterpret_cast<Luau::CompileConstant*>(&result));
                    }
                }
            }
        }
        else if (AstExprIndexExpr* expr = node->as<AstExprIndexExpr>())
        {
            analyze(expr->expr);
            analyze(expr->index);
        }
        else if (AstExprFunction* expr = node->as<AstExprFunction>())
        {
            expr->body->visit(this);
        }
        else if (AstExprTable* expr = node->as<AstExprTable>())
        {
            for (size_t i = 0; i < expr->items.size; ++i)
            {
                const AstExprTable::Item& item = expr->items.data[i];

                if (item.key)
                    analyze(item.key);

                analyze(item.value);
            }
        }
        else if (AstExprUnary* expr = node->as<AstExprUnary>())
        {
            Constant arg = analyze(expr->expr);

            if (arg.type != Constant::Type_Unknown)
                foldUnary(result, expr->op, arg);
        }
        else if (AstExprBinary* expr = node->as<AstExprBinary>())
        {
            Constant la = analyze(expr->left);
            Constant ra = analyze(expr->right);

            if (la.type != Constant::Type_Unknown)
                foldBinary(result, expr->op, la, ra, stringTable);
        }
        else if (AstExprTypeAssertion* expr = node->as<AstExprTypeAssertion>())
        {
            Constant arg = analyze(expr->expr);

            result = arg;
        }
        else if (AstExprIfElse* expr = node->as<AstExprIfElse>())
        {
            Constant cond = analyze(expr->condition);
            Constant trueExpr = analyze(expr->trueExpr);
            Constant falseExpr = analyze(expr->falseExpr);

            if (cond.type != Constant::Type_Unknown)
                result = cond.isTruthful() ? trueExpr : falseExpr;
        }
        else if (AstExprInterpString* expr = node->as<AstExprInterpString>())
        {
            bool onlyConstantSubExpr = true;
            for (AstExpr* expression : expr->expressions)
                if (analyze(expression).type != Constant::Type_String)
                    onlyConstantSubExpr = false;

            if (onlyConstantSubExpr)
                foldInterpString(result, expr, constants, stringTable);
        }
        else if (AstExprInstantiate* expr = node->as<AstExprInstantiate>())
        {
            LUAU_ASSERT(FFlag::LuauExplicitTypeInstantiationSyntax);
            result = analyze(expr->expr);
        }
        else
        {
            LUAU_ASSERT(!"Unknown expression type");
        }

        recordConstant(constants, node, result);

        return result;
    }

    template<typename T>
    void recordConstant(DenseHashMap<T, Constant>& map, T key, const Constant& value)
    {
        if (value.type != Constant::Type_Unknown)
            map[key] = value;
        else if (wasEmpty)
            ;
        else if (Constant* old = map.find(key))
            old->type = Constant::Type_Unknown;
    }

    void recordValue(AstLocal* local, const Constant& value)
    {
        Variable* v = variables.find(local);
        LUAU_ASSERT(v);

        if (!v->written)
        {
            v->constant = (value.type != Constant::Type_Unknown);
            recordConstant(locals, local, value);
        }
    }

    bool visit(AstExpr* node) override
    {
        analyze(node);

        return false;
    }

    bool visit(AstStatLocal* node) override
    {
        for (size_t i = 0; i < node->vars.size && i < node->values.size; ++i)
        {
            Constant arg = analyze(node->values.data[i]);

            recordValue(node->vars.data[i], arg);
        }

        if (node->vars.size > node->values.size)
        {
            AstExpr* last = node->values.size ? node->values.data[node->values.size - 1] : nullptr;
            bool multRet = last && (last->is<AstExprCall>() || last->is<AstExprVarargs>());

            if (!multRet)
            {
                for (size_t i = node->values.size; i < node->vars.size; ++i)
                {
                    Constant nil = {Constant::Type_Nil};
                    recordValue(node->vars.data[i], nil);
                }
            }
        }
        else
        {
            for (size_t i = node->vars.size; i < node->values.size; ++i)
                analyze(node->values.data[i]);
        }

        return false;
    }
};

void foldConstants(
    DenseHashMap<AstExpr*, Constant>& constants,
    DenseHashMap<AstLocal*, Variable>& variables,
    DenseHashMap<AstLocal*, Constant>& locals,
    const DenseHashMap<AstExprCall*, int>* builtins,
    bool foldLibraryK,
    LibraryMemberConstantCallback libraryMemberConstantCb,
    AstNode* root,
    AstNameTable& stringTable
)
{
    ConstantVisitor visitor{constants, variables, locals, builtins, foldLibraryK, libraryMemberConstantCb, stringTable};
    root->visit(&visitor);
}

}
}
