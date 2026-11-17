#include "Luau/Compiler.h"

#include "Luau/Parser.h"
#include "Luau/BytecodeBuilder.h"
#include "Luau/Common.h"
#include "Luau/TimeTrace.h"

#include "Builtins.h"
#include "ConstantFolding.h"
#include "CostModel.h"
#include "TableShape.h"
#include "Types.h"
#include "Utils.h"
#include "ValueTracking.h"

#include <algorithm>
#include <bitset>
#include <memory>

#include <math.h>

#ifdef LUAU_NO_EXCEPTIONS
#include <optional>
#include <setjmp.h>
#include <stdlib.h>
#endif

LUAU_FASTINTVARIABLE(LuauCompileLoopUnrollThreshold, 25)
LUAU_FASTINTVARIABLE(LuauCompileLoopUnrollThresholdMaxBoost, 300)

LUAU_FASTINTVARIABLE(LuauCompileInlineThreshold, 25)
LUAU_FASTINTVARIABLE(LuauCompileInlineThresholdMaxBoost, 300)
LUAU_FASTINTVARIABLE(LuauCompileInlineDepth, 5)

LUAU_FASTFLAG(LuauExplicitTypeInstantiationSyntax)
LUAU_FASTFLAGVARIABLE(LuauCompileCallCostModel)
LUAU_FASTFLAGVARIABLE(LuauCompileNoInterpStorage)
LUAU_FASTFLAGVARIABLE(LuauCompileInlineInitializers)
LUAU_FASTFLAGVARIABLE(LuauCompileExtraTableHints)
LUAU_FASTFLAGVARIABLE(LuauCompileInlinedBuiltins)

namespace Luau
{

using namespace Luau::Compile;

static const uint32_t kMaxRegisterCount = 255;
static const uint32_t kMaxUpvalueCount = 200;
static const uint32_t kMaxLocalCount = 200;
static const uint32_t kMaxInstructionCount = 1'000'000'000;

static const uint8_t kInvalidReg = 255;

static const uint32_t kDefaultAllocPc = ~0u;

void escapeAndAppend(std::string& buffer, const char* str, size_t len)
{
    if (memchr(str, '%', len))
    {
        for (size_t characterIndex = 0; characterIndex < len; ++characterIndex)
        {
            char character = str[characterIndex];
            buffer.push_back(character);

            if (character == '%')
                buffer.push_back('%');
        }
    }
    else
        buffer.append(str, len);
}

CompileError::CompileError(const Location& location, std::string message)
    : location(location)
    , message(std::move(message))
{
}

CompileError::~CompileError() throw() {}

const char* CompileError::what() const throw()
{
    return message.c_str();
}

const Location& CompileError::getLocation() const
{
    return location;
}

#ifdef LUAU_NO_EXCEPTIONS
struct CompileErrorJump
{
    jmp_buf buf;
    std::optional<CompileError> error;
    CompileErrorJump* prev = nullptr;
};

static CompileErrorJump* compileErrorJump = nullptr;

[[noreturn]] static void throwCompileError(const Location& location, std::string message)
{
    if (!compileErrorJump)
        abort();

    compileErrorJump->error.emplace(location, std::move(message));
    longjmp(compileErrorJump->buf, 1);
}

LUAU_NOINLINE static bool runCompileProtected(CompileErrorJump& jump, void (*body)(void*, void*), void* a, void* b)
{
    void (*volatile call)(void*, void*) = body;

    jump.prev = compileErrorJump;
    compileErrorJump = &jump;

    bool completed = setjmp(jump.buf) == 0;
    if (completed)
        call(a, b);

    compileErrorJump = jump.prev;
    return completed;
}
#endif

LUAU_NOINLINE void CompileError::raise(const Location& location, const char* format, ...)
{
    va_list args;
    va_start(args, format);
    std::string message = vformat(format, args);
    va_end(args);

#ifdef LUAU_NO_EXCEPTIONS
    throwCompileError(location, std::move(message));
#else
    throw CompileError(location, message);
#endif
}

static BytecodeBuilder::StringRef sref(AstName name)
{
    LUAU_ASSERT(name.value);
    return {name.value, strlen(name.value)};
}

static BytecodeBuilder::StringRef sref(AstArray<char> data)
{
    LUAU_ASSERT(data.data);
    return {data.data, data.size};
}

static BytecodeBuilder::StringRef sref(AstArray<const char> data)
{
    LUAU_ASSERT(data.data);
    return {data.data, data.size};
}

struct Compiler
{
    struct RegScope;

    Compiler(BytecodeBuilder& bytecode, const CompileOptions& options, AstNameTable& names)
        : bytecode(bytecode)
        , options(options)
        , functions(nullptr)
        , locals(nullptr)
        , globals(AstName())
        , variables(nullptr)
        , constants(nullptr)
        , locstants(nullptr)
        , tableShapes(nullptr)
        , builtins(nullptr)
        , userdataTypes(AstName())
        , functionTypes(nullptr)
        , localTypes(nullptr)
        , exprTypes(nullptr)
        , builtinTypes(options.vectorType)
        , names(names)
    {
        localStack.reserve(16);
        upvals.reserve(16);
    }

    int getLocalReg(AstLocal* local)
    {
        Local* l = locals.find(local);

        return l && l->allocated ? l->reg : -1;
    }

    uint8_t getUpval(AstLocal* local)
    {
        for (size_t uid = 0; uid < upvals.size(); ++uid)
            if (upvals[uid] == local)
                return uint8_t(uid);

        if (upvals.size() >= kMaxUpvalueCount)
            CompileError::raise(
                local->location, "Out of upvalue registers when trying to allocate %s: exceeded limit %d", local->name.value, kMaxUpvalueCount
            );

        Variable* v = variables.find(local);

        if (v && v->written)
            locals[local].captured = true;

        upvals.push_back(local);

        return uint8_t(upvals.size() - 1);
    }

    bool alwaysTerminates(AstStat* node)
    {
        if (FFlag::LuauCompileCallCostModel)
        {
            return Compile::alwaysTerminates(constants, node);
        }
        else
        {
            if (AstStatBlock* stat = node->as<AstStatBlock>())
                return stat->body.size > 0 && alwaysTerminates(stat->body.data[stat->body.size - 1]);
            else if (node->is<AstStatReturn>())
                return true;
            else if (node->is<AstStatBreak>() || node->is<AstStatContinue>())
                return true;
            else if (AstStatIf* stat = node->as<AstStatIf>())
                return stat->elsebody && alwaysTerminates(stat->thenbody) && alwaysTerminates(stat->elsebody);
            else
                return false;
        }
    }

    void emitLoadK(uint8_t target, int32_t cid)
    {
        LUAU_ASSERT(cid >= 0);

        if (cid < 32768)
        {
            bytecode.emitAD(LOP_LOADK, target, int16_t(cid));
        }
        else
        {
            bytecode.emitAD(LOP_LOADKX, target, 0);
            bytecode.emitAux(cid);
        }
    }

    AstExprFunction* getFunctionExpr(AstExpr* node)
    {
        if (AstExprLocal* expr = node->as<AstExprLocal>())
        {
            Variable* lv = variables.find(expr->local);

            if (!lv || lv->written || !lv->init)
                return nullptr;

            return getFunctionExpr(lv->init);
        }
        else if (AstExprGroup* expr = node->as<AstExprGroup>())
            return getFunctionExpr(expr->expr);
        else if (AstExprTypeAssertion* expr = node->as<AstExprTypeAssertion>())
            return getFunctionExpr(expr->expr);
        else
            return node->as<AstExprFunction>();
    }

    uint32_t compileFunction(AstExprFunction* func, uint8_t& protoflags)
    {
        LUAU_TIMETRACE_SCOPE("Compiler::compileFunction", "Compiler");

        if (func->debugname.value)
            LUAU_TIMETRACE_ARGUMENT("name", func->debugname.value);

        LUAU_ASSERT(!functions.contains(func));
        LUAU_ASSERT(regTop == 0 && stackSize == 0 && localStack.empty() && upvals.empty());

        RegScope rs(this);

        bool self = func->self != 0;
        uint32_t fid = bytecode.beginFunction(uint8_t(self + func->args.size), func->vararg);

        setDebugLine(func);

        if (func->vararg)
            bytecode.emitABC(LOP_PREPVARARGS, uint8_t(self + func->args.size), 0, 0);

        uint8_t args = allocReg(func, self + unsigned(func->args.size));

        if (func->self)
            pushLocal(func->self, args, kDefaultAllocPc);

        for (size_t i = 0; i < func->args.size; ++i)
            pushLocal(func->args.data[i], uint8_t(args + self + i), kDefaultAllocPc);

        argCount = localStack.size();

        AstStatBlock* stat = func->body;

        if (FFlag::LuauCompileCallCostModel)
        {
            bool terminatesEarly = false;

            for (size_t i = 0; i < stat->body.size; ++i)
            {
                AstStat* bodyStat = stat->body.data[i];
                compileStat(bodyStat);

                if (alwaysTerminates(bodyStat))
                {
                    terminatesEarly = true;
                    break;
                }
            }

            if (!terminatesEarly)
            {
                setDebugLineEnd(stat);
                closeLocals(0);

                bytecode.emitABC(LOP_RETURN, 0, 1, 0);
            }
        }
        else
        {
            for (size_t i = 0; i < stat->body.size; ++i)
                compileStat(stat->body.data[i]);

            if (!alwaysTerminates(stat))
            {
                setDebugLineEnd(stat);
                closeLocals(0);

                bytecode.emitABC(LOP_RETURN, 0, 1, 0);
            }
        }

        if (options.optimizationLevel >= 1 && options.debugLevel >= 2)
            gatherConstUpvals(func);

        bytecode.setDebugFunctionLineDefined(func->location.begin.line + 1);

        if (options.debugLevel >= 1 && func->debugname.value)
            bytecode.setDebugFunctionName(sref(func->debugname));

        if (options.debugLevel >= 2 && !upvals.empty())
        {
            for (AstLocal* l : upvals)
                bytecode.pushDebugUpval(sref(l->name));
        }

        if (options.typeInfoLevel >= 1)
        {
            for (AstLocal* l : upvals)
            {
                LuauBytecodeType ty = LBC_TYPE_ANY;

                if (LuauBytecodeType* recordedTy = localTypes.find(l))
                    ty = *recordedTy;

                bytecode.pushUpvalTypeInfo(ty);
            }
        }

        if (options.optimizationLevel >= 1)
            bytecode.foldJumps();

        bytecode.expandJumps();

        popLocals(0);

        if (bytecode.getInstructionCount() > kMaxInstructionCount)
            CompileError::raise(func->location, "Exceeded function instruction limit; split the function into parts to compile");

        if (std::string* funcType = functionTypes.find(func))
            bytecode.setFunctionTypeInfo(std::move(*funcType));

        if (func->functionDepth == 0 && !hasLoops)
            protoflags |= LPF_NATIVE_COLD;

        if (func->hasNativeAttribute())
            protoflags |= LPF_NATIVE_FUNCTION;

        bytecode.endFunction(uint8_t(stackSize), uint8_t(upvals.size()), protoflags);

        Function& f = functions[func];
        f.id = fid;
        f.upvals = upvals;

        if (options.optimizationLevel >= 2 && !func->vararg && !func->self && !getfenvUsed && !setfenvUsed)
        {
            f.canInline = true;
            f.stackSize = stackSize;
            f.costModel = modelCost(func->body, func->args.data, func->args.size, builtins, constants);

            if (alwaysTerminates(func->body))
            {
                ReturnVisitor returnVisitor(this);
                stat->visit(&returnVisitor);
                f.returnsOne = returnVisitor.returnsOne;
            }
        }

        upvals.clear();
        stackSize = 0;

        argCount = 0;

        hasLoops = false;

        return fid;
    }

    bool isExprMultRet(AstExpr* node)
    {
        AstExprCall* expr = node->as<AstExprCall>();
        if (!expr)
            return node->is<AstExprVarargs>();

        if (options.optimizationLevel <= 1)
            return true;

        if (isConstant(expr))
            return false;

        if (options.optimizationLevel >= 2)
        {
            if (FFlag::LuauCompileInlinedBuiltins)
            {
                if (int* bfid = builtins.find(expr); bfid && *bfid != LBF_NONE)
                    return getBuiltinInfo(*bfid).results != 1;
            }
            else
            {
                if (int* bfid = builtins.find(expr))
                    return getBuiltinInfo(*bfid).results != 1;
            }
        }

        AstExprFunction* func = getFunctionExpr(expr->func);
        Function* fi = func ? functions.find(func) : nullptr;

        if (fi && fi->returnsOne)
            return false;

        return true;
    }

    bool compileExprTempMultRet(AstExpr* node, uint8_t target)
    {
        if (AstExprCall* expr = node->as<AstExprCall>())
        {
            if (options.optimizationLevel >= 2 && !isExprMultRet(node))
            {
                compileExprTemp(node, target);
                return false;
            }

            RegScope rs(this, target);
            compileExprCall(expr, target, 0, true, true);
            return true;
        }
        else if (AstExprVarargs* expr = node->as<AstExprVarargs>())
        {
            RegScope rs(this, target);
            compileExprVarargs(expr, target, 0, true);
            return true;
        }
        else
        {
            compileExprTemp(node, target);
            return false;
        }
    }

    void compileExprTempTop(AstExpr* node, uint8_t target)
    {
        RegScope rs(this, target + 1);
        compileExprTemp(node, target);
    }

    void compileExprVarargs(AstExprVarargs* expr, uint8_t target, uint8_t targetCount, bool multRet = false)
    {
        LUAU_ASSERT(targetCount < 255);
        LUAU_ASSERT(!multRet || unsigned(target + targetCount) == regTop);

        setDebugLine(expr);

        bytecode.emitABC(LOP_GETVARARGS, target, multRet ? 0 : uint8_t(targetCount + 1), 0);
    }

    void compileExprSelectVararg(AstExprCall* expr, uint8_t target, uint8_t targetCount, bool targetTop, bool multRet, uint8_t regs)
    {
        LUAU_ASSERT(targetCount == 1);
        LUAU_ASSERT(!expr->self);
        LUAU_ASSERT(expr->args.size == 2 && expr->args.data[1]->is<AstExprVarargs>());

        AstExpr* arg = expr->args.data[0];

        uint8_t argreg;

        if (int reg = getExprLocalReg(arg); reg >= 0)
            argreg = uint8_t(reg);
        else
        {
            argreg = uint8_t(regs + 1);
            compileExprTempTop(arg, argreg);
        }

        size_t fastcallLabel = bytecode.emitLabel();

        bytecode.emitABC(LOP_FASTCALL1, LBF_SELECT_VARARG, argreg, 0);

        compileExprTemp(expr->func, regs);

        if (argreg != regs + 1)
            bytecode.emitABC(LOP_MOVE, uint8_t(regs + 1), argreg, 0);

        bytecode.emitABC(LOP_GETVARARGS, uint8_t(regs + 2), 0, 0);

        size_t callLabel = bytecode.emitLabel();
        if (!bytecode.patchSkipC(fastcallLabel, callLabel))
            CompileError::raise(expr->func->location, "Exceeded jump distance limit; simplify the code to compile");

        bytecode.emitABC(LOP_CALL, regs, 0, multRet ? 0 : uint8_t(targetCount + 1));

        if (!targetTop)
        {
            for (size_t i = 0; i < targetCount; ++i)
                bytecode.emitABC(LOP_MOVE, uint8_t(target + i), uint8_t(regs + i), 0);
        }
    }

    void compileExprFastcallN(
        AstExprCall* expr,
        uint8_t target,
        uint8_t targetCount,
        bool targetTop,
        bool multRet,
        uint8_t regs,
        int bfid,
        int bfK = -1
    )
    {
        LUAU_ASSERT(!expr->self);
        LUAU_ASSERT(expr->args.size >= 1);
        LUAU_ASSERT(expr->args.size <= 3);
        LUAU_ASSERT(bfid == LBF_BIT32_EXTRACTK ? bfK >= 0 : bfK < 0);
        LUAU_ASSERT(targetCount < 255);

        LuauOpcode opc = LOP_NOP;

        if (expr->args.size == 1)
            opc = LOP_FASTCALL1;
        else if (bfK >= 0 || (expr->args.size == 2 && isConstant(expr->args.data[1])))
            opc = LOP_FASTCALL2K;
        else if (expr->args.size == 2)
            opc = LOP_FASTCALL2;
        else
            opc = LOP_FASTCALL3;

        uint32_t args[3] = {};

        for (size_t i = 0; i < expr->args.size; ++i)
        {
            if (i > 0 && opc == LOP_FASTCALL2K)
            {
                int32_t cid = getConstantIndex(expr->args.data[i]);
                if (cid < 0)
                    CompileError::raise(expr->location, "Exceeded constant limit; simplify the code to compile");

                args[i] = cid;
            }
            else if (int reg = getExprLocalReg(expr->args.data[i]); reg >= 0)
            {
                args[i] = uint8_t(reg);
            }
            else
            {
                args[i] = uint8_t(regs + 1 + i);
                compileExprTempTop(expr->args.data[i], uint8_t(args[i]));
            }
        }

        size_t fastcallLabel = bytecode.emitLabel();

        bytecode.emitABC(opc, uint8_t(bfid), uint8_t(args[0]), 0);

        if (opc == LOP_FASTCALL3)
        {
            LUAU_ASSERT(bfK < 0);
            bytecode.emitAux(args[1] | (args[2] << 8));
        }
        else if (opc != LOP_FASTCALL1)
        {
            bytecode.emitAux(bfK >= 0 ? bfK : args[1]);
        }

        for (size_t i = 0; i < expr->args.size; ++i)
        {
            if (i > 0 && opc == LOP_FASTCALL2K)
                emitLoadK(uint8_t(regs + 1 + i), args[i]);
            else if (args[i] != regs + 1 + i)
                bytecode.emitABC(LOP_MOVE, uint8_t(regs + 1 + i), uint8_t(args[i]), 0);
        }

        compileExprTemp(expr->func, regs);

        size_t callLabel = bytecode.emitLabel();

        if (!bytecode.patchSkipC(fastcallLabel, callLabel))
            CompileError::raise(expr->func->location, "Exceeded jump distance limit; simplify the code to compile");

        bytecode.emitABC(LOP_CALL, regs, uint8_t(expr->args.size + 1), multRet ? 0 : uint8_t(targetCount + 1));

        if (!targetTop)
        {
            for (size_t i = 0; i < targetCount; ++i)
                bytecode.emitABC(LOP_MOVE, uint8_t(target + i), uint8_t(regs + i), 0);
        }
    }

    bool tryCompileInlinedCall(
        AstExprCall* expr,
        AstExprFunction* func,
        uint8_t target,
        uint8_t targetCount,
        bool multRet,
        int thresholdBase,
        int thresholdMaxBoost,
        int depthLimit
    )
    {
        Function* fi = functions.find(func);
        LUAU_ASSERT(fi);

        if (regTop > 128 || fi->stackSize > 32)
        {
            bytecode.addDebugRemark("inlining failed: high register pressure");
            return false;
        }

        if (int(inlineFrames.size()) >= depthLimit)
        {
            bytecode.addDebugRemark("inlining failed: too many inlined frames");
            return false;
        }

        for (InlineFrame& frame : inlineFrames)
            if (frame.func == func)
            {
                bytecode.addDebugRemark("inlining failed: can't inline recursive calls");
                return false;
            }

        if (multRet)
        {
            bytecode.addDebugRemark("inlining failed: can't convert fixed returns to multret");
            return false;
        }

        bool varc[8] = {};
        bool hasConstant = false;
        for (size_t i = 0; i < func->args.size && i < expr->args.size && i < 8; ++i)
        {
            if (isConstant(expr->args.data[i]))
            {
                varc[i] = true;
                hasConstant = true;
            }
        }

        if (expr->args.size != 0 && !isExprMultRet(expr->args.data[expr->args.size - 1]))
        {
            for (size_t i = expr->args.size; i < func->args.size && i < 8; ++i)
            {
                varc[i] = true;
                hasConstant = true;
            }
        }

        uint64_t callCostModel = fi->costModel;

        if (FFlag::LuauCompileCallCostModel && hasConstant)
            callCostModel = costModelInlinedCall(expr, func);

        int inlinedCost = computeCost(FFlag::LuauCompileCallCostModel ? callCostModel : fi->costModel, varc, std::min(int(func->args.size), 8));
        int baselineCost = computeCost(fi->costModel, nullptr, 0) + 3;
        int inlineProfit = (inlinedCost == 0) ? thresholdMaxBoost : std::min(thresholdMaxBoost, 100 * baselineCost / inlinedCost);

        int threshold = thresholdBase * inlineProfit / 100;

        if (inlinedCost > threshold)
        {
            bytecode.addDebugRemark("inlining failed: too expensive (cost %d, profit %.2fx)", inlinedCost, double(inlineProfit) / 100);
            return false;
        }

        bytecode.addDebugRemark(
            "inlining succeeded (cost %d, profit %.2fx, depth %d)", inlinedCost, double(inlineProfit) / 100, int(inlineFrames.size())
        );

        compileInlinedCall(expr, func, target, targetCount);
        return true;
    }

    uint64_t costModelInlinedCall(AstExprCall* expr, AstExprFunction* func)
    {
        LUAU_ASSERT(FFlag::LuauCompileCallCostModel);

        for (size_t i = 0; i < func->args.size; ++i)
        {
            AstLocal* var = func->args.data[i];
            AstExpr* arg = i < expr->args.size ? expr->args.data[i] : nullptr;

            if (i + 1 == expr->args.size && func->args.size > expr->args.size && isExprMultRet(arg))
                break;

            if (Variable* vv = variables.find(var); vv && vv->written)
                continue;

            if (arg == nullptr)
                locstants[var] = {Constant::Type_Nil};
            else if (const Constant* cv = constants.find(arg); cv && cv->type != Constant::Type_Unknown)
                locstants[var] = *cv;
        }

        foldConstants(constants, variables, locstants, builtinsFold, builtinsFoldLibraryK, options.libraryMemberConstantCb, func->body, names);

        uint64_t cost = modelCost(func->body, func->args.data, func->args.size, builtins, constants);

        for (size_t i = 0; i < func->args.size; ++i)
        {
            if (Constant* var = locstants.find(func->args.data[i]))
                var->type = Constant::Type_Unknown;
        }

        foldConstants(constants, variables, locstants, builtinsFold, builtinsFoldLibraryK, options.libraryMemberConstantCb, func->body, names);

        return cost;
    }

    void compileInlinedCall(AstExprCall* expr, AstExprFunction* func, uint8_t target, uint8_t targetCount)
    {
        RegScope rs(this);

        size_t oldLocals = localStack.size();

        std::vector<InlineArg> args;
        args.reserve(func->args.size);

        for (size_t i = 0; i < func->args.size; ++i)
        {
            AstLocal* var = func->args.data[i];
            AstExpr* arg = i < expr->args.size ? expr->args.data[i] : nullptr;

            if (i + 1 == expr->args.size && func->args.size > expr->args.size && isExprMultRet(arg))
            {
                unsigned int tail = unsigned(func->args.size - expr->args.size) + 1;
                uint8_t reg = allocReg(arg, tail);
                uint32_t allocpc = bytecode.getDebugPC();

                if (AstExprCall* expr = arg->as<AstExprCall>())
                    compileExprCall(expr, reg, tail, true);
                else if (AstExprVarargs* expr = arg->as<AstExprVarargs>())
                    compileExprVarargs(expr, reg, tail);
                else
                    LUAU_ASSERT(!"Unexpected expression type");

                for (size_t j = i; j < func->args.size; ++j)
                    args.push_back({func->args.data[j], uint8_t(reg + (j - i)), {Constant::Type_Unknown}, allocpc});

                break;
            }
            else if (Variable* vv = variables.find(var); vv && vv->written)
            {
                uint8_t reg = allocReg(arg, 1u);
                uint32_t allocpc = bytecode.getDebugPC();

                if (arg)
                    compileExprTemp(arg, reg);
                else
                    bytecode.emitABC(LOP_LOADNIL, reg, 0, 0);

                args.push_back({var, reg, {Constant::Type_Unknown}, allocpc});
            }
            else if (arg == nullptr)
            {
                args.push_back({var, kInvalidReg, {Constant::Type_Nil}});
            }
            else if (const Constant* cv = constants.find(arg); cv && cv->type != Constant::Type_Unknown)
            {
                args.push_back({var, kInvalidReg, *cv});
            }
            else
            {
                AstExprLocal* le = getExprLocal(arg);
                Variable* lv = le ? variables.find(le->local) : nullptr;

                if (int reg = le ? getExprLocalReg(le) : -1; reg >= 0 && (!lv || !lv->written))
                {
                    args.push_back({var, uint8_t(reg), {Constant::Type_Unknown}, kDefaultAllocPc, lv ? lv->init : nullptr});
                }
                else
                {
                    uint8_t temp = allocReg(arg, 1u);
                    uint32_t allocpc = bytecode.getDebugPC();

                    compileExprTemp(arg, temp);

                    if (FFlag::LuauCompileInlineInitializers)
                        args.push_back({var, temp, {Constant::Type_Unknown}, allocpc, arg});
                    else
                        args.push_back({var, temp, {Constant::Type_Unknown}, allocpc});
                }
            }
        }

        for (size_t i = func->args.size; i < expr->args.size; ++i)
            compileExprSide(expr->args.data[i]);

        for (InlineArg& arg : args)
        {
            if (arg.value.type == Constant::Type_Unknown)
            {
                pushLocal(arg.local, arg.reg, arg.allocpc);

                if (arg.init)
                {
                    if (Variable* lv = variables.find(arg.local))
                        lv->init = arg.init;
                }
            }
            else
            {
                locstants[arg.local] = arg.value;
            }
        }

        inlineFrames.push_back({func, oldLocals, target, targetCount});

        if (FFlag::LuauCompileInlinedBuiltins)
        {
            analyzeBuiltins(inlineBuiltins, globals, variables, options, func->body, names);

            if (!inlineBuiltins.empty())
            {
                for (auto [callExpr, bfid] : inlineBuiltins)
                {
                    int& builtin = builtins[callExpr];

                    if (bfid != builtin)
                    {
                        inlineBuiltinsBackup[callExpr] = builtin;
                        builtin = bfid;
                    }
                }

                inlineBuiltins.clear();
            }
        }

        foldConstants(constants, variables, locstants, builtinsFold, builtinsFoldLibraryK, options.libraryMemberConstantCb, func->body, names);

        if (FFlag::LuauCompileCallCostModel)
        {
            bool terminatesEarly = false;

            for (size_t i = 0; i < func->body->body.size; ++i)
            {
                AstStat* stat = func->body->body.data[i];
                compileStat(stat);

                if (alwaysTerminates(stat))
                {
                    terminatesEarly = true;

                    InlineFrame& currFrame = inlineFrames.back();
                    if (!currFrame.returnJumps.empty() && currFrame.returnJumps.back() == bytecode.emitLabel() - 1)
                    {
                        bytecode.undoEmit(LOP_JUMP);
                        currFrame.returnJumps.pop_back();
                    }
                    break;
                }
            }

            if (!terminatesEarly)
            {
                for (size_t i = 0; i < targetCount; ++i)
                    bytecode.emitABC(LOP_LOADNIL, uint8_t(target + i), 0, 0);

                closeLocals(oldLocals);
            }
        }
        else
        {
            bool usedFallthrough = false;

            for (size_t i = 0; i < func->body->body.size; ++i)
            {
                AstStat* stat = func->body->body.data[i];

                if (AstStatReturn* ret = stat->as<AstStatReturn>())
                {
                    compileInlineReturn(ret, true);
                    usedFallthrough = true;
                    break;
                }
                else
                    compileStat(stat);
            }

            if (!usedFallthrough && !alwaysTerminates(func->body))
            {
                for (size_t i = 0; i < targetCount; ++i)
                    bytecode.emitABC(LOP_LOADNIL, uint8_t(target + i), 0, 0);

                closeLocals(oldLocals);
            }
        }

        popLocals(oldLocals);

        size_t returnLabel = bytecode.emitLabel();
        patchJumps(expr, inlineFrames.back().returnJumps, returnLabel);

        inlineFrames.pop_back();

        for (size_t i = 0; i < func->args.size; ++i)
        {
            AstLocal* local = func->args.data[i];

            if (Constant* var = locstants.find(local))
                var->type = Constant::Type_Unknown;

            if (Variable* lv = variables.find(local))
                lv->init = nullptr;
        }

        if (FFlag::LuauCompileInlinedBuiltins)
        {
            if (!inlineBuiltinsBackup.empty())
            {
                for (auto [callExpr, bfid] : inlineBuiltinsBackup)
                    builtins[callExpr] = bfid;

                inlineBuiltinsBackup.clear();
            }
        }

        foldConstants(constants, variables, locstants, builtinsFold, builtinsFoldLibraryK, options.libraryMemberConstantCb, func->body, names);
    }

    void compileExprCall(AstExprCall* expr, uint8_t target, uint8_t targetCount, bool targetTop = false, bool multRet = false)
    {
        LUAU_ASSERT(targetCount < 255);
        LUAU_ASSERT(!targetTop || unsigned(target + targetCount) == regTop);

        setDebugLine(expr);

        if (options.optimizationLevel >= 2 && !expr->self)
        {
            AstExprFunction* func = getFunctionExpr(expr->func);
            Function* fi = func ? functions.find(func) : nullptr;

            if (fi && fi->canInline &&
                tryCompileInlinedCall(
                    expr,
                    func,
                    target,
                    targetCount,
                    multRet,
                    FInt::LuauCompileInlineThreshold,
                    FInt::LuauCompileInlineThresholdMaxBoost,
                    FInt::LuauCompileInlineDepth
                ))
                return;

            if (func && !(fi && fi->canInline))
            {
                if (func->vararg)
                    bytecode.addDebugRemark("inlining failed: function is variadic");
                else if (!fi)
                    bytecode.addDebugRemark("inlining failed: can't inline recursive calls");
                else if (getfenvUsed || setfenvUsed)
                    bytecode.addDebugRemark("inlining failed: module uses getfenv/setfenv");
            }
        }

        RegScope rs(this);

        unsigned int regCount = std::max(unsigned(1 + expr->self + expr->args.size), unsigned(targetCount));

        uint8_t regs = targetTop ? allocReg(expr, regCount - targetCount) - targetCount : allocReg(expr, regCount);

        uint8_t selfreg = 0;

        int bfid = -1;

        if (options.optimizationLevel >= 1 && !expr->self)
        {
            if (FFlag::LuauCompileInlinedBuiltins)
            {
                if (const int* id = builtins.find(expr); id && *id != LBF_NONE)
                    bfid = *id;
            }
            else
            {
                if (const int* id = builtins.find(expr))
                    bfid = *id;
            }
        }

        if (bfid >= 0 && bytecode.needsDebugRemarks())
        {
            Builtin builtin = getBuiltin(expr->func, globals, variables);
            bool lastMult = expr->args.size > 0 && isExprMultRet(expr->args.data[expr->args.size - 1]);

            if (builtin.object.value)
                bytecode.addDebugRemark("builtin %s.%s/%d%s", builtin.object.value, builtin.method.value, int(expr->args.size), lastMult ? "+" : "");
            else if (builtin.method.value)
                bytecode.addDebugRemark("builtin %s/%d%s", builtin.method.value, int(expr->args.size), lastMult ? "+" : "");
        }

        if (bfid == LBF_SELECT_VARARG)
        {
            if (multRet == false && targetCount == 1)
                return compileExprSelectVararg(expr, target, targetCount, targetTop, multRet, regs);
            else
                bfid = -1;
        }

        if (bfid == LBF_BIT32_EXTRACT && expr->args.size == 3 && isConstant(expr->args.data[1]) && isConstant(expr->args.data[2]))
        {
            Constant fc = getConstant(expr->args.data[1]);
            Constant wc = getConstant(expr->args.data[2]);

            int fi = fc.type == Constant::Type_Number ? int(fc.valueNumber) : -1;
            int wi = wc.type == Constant::Type_Number ? int(wc.valueNumber) : -1;

            if (fi >= 0 && wi > 0 && fi + wi <= 32)
            {
                int fwp = fi | ((wi - 1) << 5);
                int32_t cid = bytecode.addConstantNumber(fwp);
                if (cid < 0)
                    CompileError::raise(expr->location, "Exceeded constant limit; simplify the code to compile");

                return compileExprFastcallN(expr, target, targetCount, targetTop, multRet, regs, LBF_BIT32_EXTRACTK, cid);
            }
        }

        unsigned maxFastcallArgs = 2;

        if (bfid >= 0 && expr->args.size == 3)
        {
            for (size_t i = 0; i < expr->args.size; ++i)
            {
                if (int reg = getExprLocalReg(expr->args.data[i]); reg >= 0)
                {
                    maxFastcallArgs = 3;
                    break;
                }
            }
        }

        if (bfid >= 0 && expr->args.size >= 1 && expr->args.size <= maxFastcallArgs)
        {
            if (!isExprMultRet(expr->args.data[expr->args.size - 1]))
            {
                return compileExprFastcallN(expr, target, targetCount, targetTop, multRet, regs, bfid);
            }
            else if (options.optimizationLevel >= 2)
            {
                BuiltinInfo info = getBuiltinInfo(bfid);
                if (int(expr->args.size) == info.params && (info.flags & BuiltinInfo::Flag_NoneSafe) != 0)
                    return compileExprFastcallN(expr, target, targetCount, targetTop, multRet, regs, bfid);
            }
        }

        if (expr->self)
        {
            AstExprIndexName* fi = expr->func->as<AstExprIndexName>();
            LUAU_ASSERT(fi);

            if (int reg = getExprLocalReg(fi->expr); reg >= 0)
            {
                selfreg = uint8_t(reg);
            }
            else
            {
                selfreg = regs;

                compileExprTempTop(fi->expr, selfreg);
            }
        }
        else if (bfid < 0)
        {
            compileExprTempTop(expr->func, regs);
        }

        bool multCall = false;

        for (size_t i = 0; i < expr->args.size; ++i)
            if (i + 1 == expr->args.size)
                multCall = compileExprTempMultRet(expr->args.data[i], uint8_t(regs + 1 + expr->self + i));
            else
                compileExprTempTop(expr->args.data[i], uint8_t(regs + 1 + expr->self + i));

        setDebugLineEnd(expr->func);

        if (expr->self)
        {
            AstExprIndexName* fi = expr->func->as<AstExprIndexName>();
            LUAU_ASSERT(fi);

            setDebugLine(fi->indexLocation);

            BytecodeBuilder::StringRef iname = sref(fi->index);
            int32_t cid = bytecode.addConstantString(iname);
            if (cid < 0)
                CompileError::raise(fi->location, "Exceeded constant limit; simplify the code to compile");

            bytecode.emitABC(LOP_NAMECALL, regs, selfreg, uint8_t(BytecodeBuilder::getStringHash(iname)));
            bytecode.emitAux(cid);

            hintTemporaryExprRegType(fi->expr, selfreg, LBC_TYPE_TABLE, 2);
        }
        else if (bfid >= 0)
        {
            size_t fastcallLabel = bytecode.emitLabel();
            bytecode.emitABC(LOP_FASTCALL, uint8_t(bfid), 0, 0);

            compileExprTemp(expr->func, regs);

            size_t callLabel = bytecode.emitLabel();

            if (!bytecode.patchSkipC(fastcallLabel, callLabel))
                CompileError::raise(expr->func->location, "Exceeded jump distance limit; simplify the code to compile");
        }

        bytecode.emitABC(LOP_CALL, regs, multCall ? 0 : uint8_t(expr->self + expr->args.size + 1), multRet ? 0 : uint8_t(targetCount + 1));

        if (!targetTop)
        {
            for (size_t i = 0; i < targetCount; ++i)
                bytecode.emitABC(LOP_MOVE, uint8_t(target + i), uint8_t(regs + i), 0);
        }
    }

    bool shouldShareClosure(AstExprFunction* func)
    {
        const Function* f = functions.find(func);
        if (!f)
            return false;

        for (AstLocal* uv : f->upvals)
        {
            Variable* ul = variables.find(uv);

            if (!ul)
                return false;

            if (ul->written)
                return false;

            if (uv->functionDepth != 0 || uv->loopDepth != 0)
            {
                AstExprFunction* uf = ul->init ? ul->init->as<AstExprFunction>() : nullptr;
                if (!uf)
                    return false;

                if (uf != func && !shouldShareClosure(uf))
                    return false;
            }
        }

        return true;
    }

    void compileExprFunction(AstExprFunction* expr, uint8_t target)
    {
        RegScope rs(this);

        const Function* f = functions.find(expr);
        LUAU_ASSERT(f);

        int16_t pid = bytecode.addChildFunction(f->id);
        if (pid < 0)
            CompileError::raise(expr->location, "Exceeded closure limit; simplify the code to compile");

        captures.clear();
        captures.reserve(f->upvals.size());

        for (AstLocal* uv : f->upvals)
        {
            LUAU_ASSERT(uv->functionDepth < expr->functionDepth);

            if (int reg = getLocalReg(uv); reg >= 0)
            {
                Variable* ul = variables.find(uv);
                bool immutable = !ul || !ul->written;

                captures.push_back({immutable ? LCT_VAL : LCT_REF, uint8_t(reg)});
            }
            else if (const Constant* uc = locstants.find(uv); uc && uc->type != Constant::Type_Unknown)
            {
                uint8_t reg = allocReg(expr, 1u);
                compileExprConstant(expr, uc, reg);

                captures.push_back({LCT_VAL, reg});
            }
            else
            {
                LUAU_ASSERT(uv->functionDepth < expr->functionDepth - 1);

                uint8_t uid = getUpval(uv);

                captures.push_back({LCT_UPVAL, uid});
            }
        }

        int16_t shared = -1;

        if (options.optimizationLevel >= 1 && shouldShareClosure(expr) && !setfenvUsed)
        {
            int32_t cid = bytecode.addConstantClosure(f->id);

            if (cid >= 0 && cid < 32768)
                shared = int16_t(cid);
        }

        if (shared < 0)
            bytecode.addDebugRemark("allocation: closure with %d upvalues", int(captures.size()));

        if (shared >= 0)
            bytecode.emitAD(LOP_DUPCLOSURE, target, shared);
        else
            bytecode.emitAD(LOP_NEWCLOSURE, target, pid);

        for (const Capture& c : captures)
        {
            bytecode.emitABC(LOP_CAPTURE, uint8_t(c.type), c.data, 0);
        }
    }

    LuauOpcode getUnaryOp(AstExprUnary::Op op)
    {
        switch (op)
        {
        case AstExprUnary::Not:
            return LOP_NOT;

        case AstExprUnary::Minus:
            return LOP_MINUS;

        case AstExprUnary::Len:
            return LOP_LENGTH;

        default:
            LUAU_ASSERT(!"Unexpected unary operation");
            return LOP_NOP;
        }
    }

    LuauOpcode getBinaryOpArith(AstExprBinary::Op op, bool k = false)
    {
        switch (op)
        {
        case AstExprBinary::Add:
            return k ? LOP_ADDK : LOP_ADD;

        case AstExprBinary::Sub:
            return k ? LOP_SUBK : LOP_SUB;

        case AstExprBinary::Mul:
            return k ? LOP_MULK : LOP_MUL;

        case AstExprBinary::Div:
            return k ? LOP_DIVK : LOP_DIV;

        case AstExprBinary::FloorDiv:
            return k ? LOP_IDIVK : LOP_IDIV;

        case AstExprBinary::Mod:
            return k ? LOP_MODK : LOP_MOD;

        case AstExprBinary::Pow:
            return k ? LOP_POWK : LOP_POW;

        default:
            LUAU_ASSERT(!"Unexpected binary operation");
            return LOP_NOP;
        }
    }

    LuauOpcode getJumpOpCompare(AstExprBinary::Op op, bool not_ = false)
    {
        switch (op)
        {
        case AstExprBinary::CompareNe:
            return not_ ? LOP_JUMPIFEQ : LOP_JUMPIFNOTEQ;

        case AstExprBinary::CompareEq:
            return not_ ? LOP_JUMPIFNOTEQ : LOP_JUMPIFEQ;

        case AstExprBinary::CompareLt:
        case AstExprBinary::CompareGt:
            return not_ ? LOP_JUMPIFNOTLT : LOP_JUMPIFLT;

        case AstExprBinary::CompareLe:
        case AstExprBinary::CompareGe:
            return not_ ? LOP_JUMPIFNOTLE : LOP_JUMPIFLE;

        default:
            LUAU_ASSERT(!"Unexpected binary operation");
            return LOP_NOP;
        }
    }

    bool isConstant(AstExpr* node)
    {
        const Constant* cv = constants.find(node);

        return cv && cv->type != Constant::Type_Unknown;
    }

    bool isConstantTrue(AstExpr* node)
    {
        if (FFlag::LuauCompileCallCostModel)
        {
            return Compile::isConstantTrue(constants, node);
        }
        else
        {
            const Constant* cv = constants.find(node);

            return cv && cv->type != Constant::Type_Unknown && cv->isTruthful();
        }
    }

    bool isConstantFalse(AstExpr* node)
    {
        if (FFlag::LuauCompileCallCostModel)
        {
            return Compile::isConstantFalse(constants, node);
        }
        else
        {
            const Constant* cv = constants.find(node);

            return cv && cv->type != Constant::Type_Unknown && !cv->isTruthful();
        }
    }

    bool isConstantVector(AstExpr* node)
    {
        const Constant* cv = constants.find(node);

        return cv && cv->type == Constant::Type_Vector;
    }

    Constant getConstant(AstExpr* node)
    {
        const Constant* cv = constants.find(node);

        return cv ? *cv : Constant{Constant::Type_Unknown};
    }

    size_t compileCompareJump(AstExprBinary* expr, bool not_ = false)
    {
        RegScope rs(this);

        bool isEq = (expr->op == AstExprBinary::CompareEq || expr->op == AstExprBinary::CompareNe);
        AstExpr* left = expr->left;
        AstExpr* right = expr->right;

        bool operandIsConstant = isConstant(right);
        if (isEq && !operandIsConstant)
        {
            operandIsConstant = isConstant(left);
            if (operandIsConstant)
                std::swap(left, right);
        }

        if (operandIsConstant && isConstantVector(right))
            operandIsConstant = false;

        uint8_t rl = compileExprAuto(left, rs);

        if (isEq && operandIsConstant)
        {
            const Constant* cv = constants.find(right);
            LUAU_ASSERT(cv && cv->type != Constant::Type_Unknown);

            LuauOpcode opc = LOP_NOP;
            int32_t cid = -1;
            uint32_t flip = (expr->op == AstExprBinary::CompareEq) == not_ ? 0x80000000 : 0;

            switch (cv->type)
            {
            case Constant::Type_Nil:
                opc = LOP_JUMPXEQKNIL;
                cid = 0;
                break;

            case Constant::Type_Boolean:
                opc = LOP_JUMPXEQKB;
                cid = cv->valueBoolean;
                break;

            case Constant::Type_Number:
                opc = LOP_JUMPXEQKN;
                cid = getConstantIndex(right);
                break;

            case Constant::Type_String:
                opc = LOP_JUMPXEQKS;
                cid = getConstantIndex(right);
                break;

            default:
                LUAU_ASSERT(!"Unexpected constant type");
            }

            if (cid < 0)
                CompileError::raise(expr->location, "Exceeded constant limit; simplify the code to compile");

            size_t jumpLabel = bytecode.emitLabel();

            bytecode.emitAD(opc, rl, 0);
            bytecode.emitAux(cid | flip);

            return jumpLabel;
        }
        else
        {
            LuauOpcode opc = getJumpOpCompare(expr->op, not_);

            uint8_t rr = compileExprAuto(right, rs);

            size_t jumpLabel = bytecode.emitLabel();

            if (expr->op == AstExprBinary::CompareGt || expr->op == AstExprBinary::CompareGe)
            {
                bytecode.emitAD(opc, rr, 0);
                bytecode.emitAux(rl);
            }
            else
            {
                bytecode.emitAD(opc, rl, 0);
                bytecode.emitAux(rr);
            }

            return jumpLabel;
        }
    }

    int32_t getConstantNumber(AstExpr* node)
    {
        const Constant* c = constants.find(node);

        if (c && c->type == Constant::Type_Number)
        {
            int cid = bytecode.addConstantNumber(c->valueNumber);
            if (cid < 0)
                CompileError::raise(node->location, "Exceeded constant limit; simplify the code to compile");

            return cid;
        }

        return -1;
    }

    int32_t getConstantIndex(AstExpr* node)
    {
        const Constant* c = constants.find(node);

        if (!c || c->type == Constant::Type_Unknown)
            return -1;

        int cid = -1;

        switch (c->type)
        {
        case Constant::Type_Nil:
            cid = bytecode.addConstantNil();
            break;

        case Constant::Type_Boolean:
            cid = bytecode.addConstantBoolean(c->valueBoolean);
            break;

        case Constant::Type_Number:
            cid = bytecode.addConstantNumber(c->valueNumber);
            break;

        case Constant::Type_Vector:
            cid = bytecode.addConstantVector(c->valueVector[0], c->valueVector[1], c->valueVector[2], c->valueVector[3]);
            break;

        case Constant::Type_String:
            cid = bytecode.addConstantString(sref(c->getString()));
            break;

        default:
            LUAU_ASSERT(!"Unexpected constant type");
            return -1;
        }

        if (cid < 0)
            CompileError::raise(node->location, "Exceeded constant limit; simplify the code to compile");

        return cid;
    }

    void compileConditionValue(AstExpr* node, const uint8_t* target, std::vector<size_t>& skipJump, bool onlyTruth)
    {
        if (const Constant* cv = constants.find(node); cv && cv->type != Constant::Type_Unknown)
        {
            if (cv->isTruthful() == onlyTruth)
            {
                if (target)
                    compileExprTemp(node, *target);

                skipJump.push_back(bytecode.emitLabel());
                bytecode.emitAD(LOP_JUMP, 0, 0);
            }
            return;
        }

        if (AstExprBinary* expr = node->as<AstExprBinary>())
        {
            switch (expr->op)
            {
            case AstExprBinary::And:
            case AstExprBinary::Or:
            {
                if (onlyTruth == (expr->op == AstExprBinary::And))
                {
                    std::vector<size_t> elseJump;
                    compileConditionValue(expr->left, nullptr, elseJump, !onlyTruth);

                    compileConditionValue(expr->right, target, skipJump, onlyTruth);

                    size_t elseLabel = bytecode.emitLabel();

                    patchJumps(expr, elseJump, elseLabel);
                }
                else
                {
                    compileConditionValue(expr->left, target, skipJump, onlyTruth);

                    compileConditionValue(expr->right, target, skipJump, onlyTruth);
                }
                return;
            }
            break;

            case AstExprBinary::CompareNe:
            case AstExprBinary::CompareEq:
            case AstExprBinary::CompareLt:
            case AstExprBinary::CompareLe:
            case AstExprBinary::CompareGt:
            case AstExprBinary::CompareGe:
            {
                if (target)
                {
                    bytecode.emitABC(LOP_LOADB, *target, onlyTruth ? 1 : 0, 0);
                }

                size_t jumpLabel = compileCompareJump(expr, !onlyTruth);

                skipJump.push_back(jumpLabel);
                return;
            }
            break;

            default:;
            }
        }

        if (AstExprUnary* expr = node->as<AstExprUnary>())
        {
            if (!target && expr->op == AstExprUnary::Not)
            {
                compileConditionValue(expr->expr, target, skipJump, !onlyTruth);
                return;
            }
        }

        if (AstExprGroup* expr = node->as<AstExprGroup>())
        {
            compileConditionValue(expr->expr, target, skipJump, onlyTruth);
            return;
        }

        RegScope rs(this);
        uint8_t reg;

        if (target)
        {
            reg = *target;
            compileExprTemp(node, reg);
        }
        else
        {
            reg = compileExprAuto(node, rs);
        }

        skipJump.push_back(bytecode.emitLabel());
        bytecode.emitAD(onlyTruth ? LOP_JUMPIF : LOP_JUMPIFNOT, reg, 0);
    }

    bool isConditionFast(AstExpr* node)
    {
        const Constant* cv = constants.find(node);

        if (cv && cv->type != Constant::Type_Unknown)
            return true;

        if (AstExprBinary* expr = node->as<AstExprBinary>())
        {
            switch (expr->op)
            {
            case AstExprBinary::And:
            case AstExprBinary::Or:
                return true;

            case AstExprBinary::CompareNe:
            case AstExprBinary::CompareEq:
            case AstExprBinary::CompareLt:
            case AstExprBinary::CompareLe:
            case AstExprBinary::CompareGt:
            case AstExprBinary::CompareGe:
                return true;

            default:
                return false;
            }
        }

        if (AstExprGroup* expr = node->as<AstExprGroup>())
            return isConditionFast(expr->expr);

        return false;
    }

    void compileExprAndOr(AstExprBinary* expr, uint8_t target, bool targetTemp)
    {
        bool and_ = (expr->op == AstExprBinary::And);

        RegScope rs(this);

        if (const Constant* cl = constants.find(expr->left); cl && cl->type != Constant::Type_Unknown)
        {
            compileExpr(and_ == cl->isTruthful() ? expr->right : expr->left, target, targetTemp);
            return;
        }

        if (!isConditionFast(expr->left))
        {
            if (int reg = getExprLocalReg(expr->right); reg >= 0)
            {
                uint8_t lr = compileExprAuto(expr->left, rs);
                uint8_t rr = uint8_t(reg);

                bytecode.emitABC(and_ ? LOP_AND : LOP_OR, target, lr, rr);
                return;
            }

            int32_t cid = getConstantIndex(expr->right);

            if (cid >= 0 && cid <= 255)
            {
                uint8_t lr = compileExprAuto(expr->left, rs);

                bytecode.emitABC(and_ ? LOP_ANDK : LOP_ORK, target, lr, uint8_t(cid));
                return;
            }
        }

        uint8_t reg = targetTemp ? target : allocReg(expr, 1u);

        std::vector<size_t> skipJump;
        compileConditionValue(expr->left, &reg, skipJump, !and_);

        compileExprTemp(expr->right, reg);

        size_t moveLabel = bytecode.emitLabel();

        patchJumps(expr, skipJump, moveLabel);

        if (target != reg)
            bytecode.emitABC(LOP_MOVE, target, reg, 0);
    }

    void compileExprUnary(AstExprUnary* expr, uint8_t target)
    {
        RegScope rs(this);

        uint8_t re = compileExprAuto(expr->expr, rs);

        bytecode.emitABC(getUnaryOp(expr->op), target, re, 0);
    }

    static void unrollConcats(std::vector<AstExpr*>& args)
    {
        for (;;)
        {
            AstExprBinary* be = args.back()->as<AstExprBinary>();

            if (!be || be->op != AstExprBinary::Concat)
                break;

            args.back() = be->left;
            args.push_back(be->right);
        }
    }

    void compileExprBinary(AstExprBinary* expr, uint8_t target, bool targetTemp)
    {
        RegScope rs(this);

        switch (expr->op)
        {
        case AstExprBinary::Add:
        case AstExprBinary::Sub:
        case AstExprBinary::Mul:
        case AstExprBinary::Div:
        case AstExprBinary::FloorDiv:
        case AstExprBinary::Mod:
        case AstExprBinary::Pow:
        {
            int32_t rc = getConstantNumber(expr->right);

            if (rc >= 0 && rc <= 255)
            {
                uint8_t rl = compileExprAuto(expr->left, rs);

                bytecode.emitABC(getBinaryOpArith(expr->op, true), target, rl, uint8_t(rc));

                hintTemporaryExprRegType(expr->left, rl, LBC_TYPE_NUMBER, 1);
            }
            else
            {
                if (expr->op == AstExprBinary::Sub || expr->op == AstExprBinary::Div)
                {
                    int32_t lc = getConstantNumber(expr->left);

                    if (lc >= 0 && lc <= 255)
                    {
                        uint8_t rr = compileExprAuto(expr->right, rs);
                        LuauOpcode op = (expr->op == AstExprBinary::Sub) ? LOP_SUBRK : LOP_DIVRK;

                        bytecode.emitABC(op, target, uint8_t(lc), uint8_t(rr));

                        hintTemporaryExprRegType(expr->right, rr, LBC_TYPE_NUMBER, 1);
                        return;
                    }
                }
                else if (options.optimizationLevel >= 2 && (expr->op == AstExprBinary::Add || expr->op == AstExprBinary::Mul))
                {
                    if (LuauBytecodeType* ty = exprTypes.find(expr); ty && *ty == LBC_TYPE_NUMBER)
                    {
                        int32_t lc = getConstantNumber(expr->left);

                        if (lc >= 0 && lc <= 255)
                        {
                            uint8_t rr = compileExprAuto(expr->right, rs);

                            bytecode.emitABC(getBinaryOpArith(expr->op, true), target, rr, uint8_t(lc));

                            hintTemporaryExprRegType(expr->right, rr, LBC_TYPE_NUMBER, 1);
                            return;
                        }
                    }
                }

                uint8_t rl = compileExprAuto(expr->left, rs);
                uint8_t rr = compileExprAuto(expr->right, rs);

                bytecode.emitABC(getBinaryOpArith(expr->op), target, rl, rr);

                hintTemporaryExprRegType(expr->left, rl, LBC_TYPE_NUMBER, 1);
                hintTemporaryExprRegType(expr->right, rr, LBC_TYPE_NUMBER, 1);
            }
        }
        break;

        case AstExprBinary::Concat:
        {
            std::vector<AstExpr*> args = {expr->left, expr->right};

            unrollConcats(args);

            uint8_t regs = allocReg(expr, unsigned(args.size()));

            for (size_t i = 0; i < args.size(); ++i)
                compileExprTemp(args[i], uint8_t(regs + i));

            bytecode.emitABC(LOP_CONCAT, target, regs, uint8_t(regs + args.size() - 1));
        }
        break;

        case AstExprBinary::CompareNe:
        case AstExprBinary::CompareEq:
        case AstExprBinary::CompareLt:
        case AstExprBinary::CompareLe:
        case AstExprBinary::CompareGt:
        case AstExprBinary::CompareGe:
        {
            size_t jumpLabel = compileCompareJump(expr);

            bytecode.emitABC(LOP_LOADB, target, 0, 1);

            size_t thenLabel = bytecode.emitLabel();

            bytecode.emitABC(LOP_LOADB, target, 1, 0);

            patchJump(expr, jumpLabel, thenLabel);
        }
        break;

        case AstExprBinary::And:
        case AstExprBinary::Or:
        {
            compileExprAndOr(expr, target, targetTemp);
        }
        break;

        default:
            LUAU_ASSERT(!"Unexpected binary operation");
        }
    }

    void compileExprIfElseAndOr(bool and_, uint8_t creg, AstExpr* other, uint8_t target)
    {
        int32_t cid = getConstantIndex(other);

        if (cid >= 0 && cid <= 255)
        {
            bytecode.emitABC(and_ ? LOP_ANDK : LOP_ORK, target, creg, uint8_t(cid));
        }
        else
        {
            RegScope rs(this);
            uint8_t oreg = compileExprAuto(other, rs);

            bytecode.emitABC(and_ ? LOP_AND : LOP_OR, target, creg, oreg);
        }
    }

    void compileExprIfElse(AstExprIfElse* expr, uint8_t target, bool targetTemp)
    {
        if (isConstant(expr->condition))
        {
            if (isConstantTrue(expr->condition))
            {
                compileExpr(expr->trueExpr, target, targetTemp);
            }
            else
            {
                compileExpr(expr->falseExpr, target, targetTemp);
            }
        }
        else
        {
            if (int creg = getExprLocalReg(expr->condition); creg >= 0)
            {
                if (creg == getExprLocalReg(expr->trueExpr) && (getExprLocalReg(expr->falseExpr) >= 0 || isConstant(expr->falseExpr)))
                    return compileExprIfElseAndOr( false, uint8_t(creg), expr->falseExpr, target);
                else if (creg == getExprLocalReg(expr->falseExpr) && (getExprLocalReg(expr->trueExpr) >= 0 || isConstant(expr->trueExpr)))
                    return compileExprIfElseAndOr( true, uint8_t(creg), expr->trueExpr, target);
            }

            std::vector<size_t> elseJump;
            compileConditionValue(expr->condition, nullptr, elseJump, false);
            compileExpr(expr->trueExpr, target, targetTemp);

            size_t thenLabel = bytecode.emitLabel();
            bytecode.emitAD(LOP_JUMP, 0, 0);

            size_t elseLabel = bytecode.emitLabel();
            compileExpr(expr->falseExpr, target, targetTemp);
            size_t endLabel = bytecode.emitLabel();

            patchJumps(expr, elseJump, elseLabel);
            patchJump(expr, thenLabel, endLabel);
        }
    }

    void compileExprInterpString(AstExprInterpString* expr, uint8_t target, bool targetTemp)
    {
        size_t formatCapacity = 0;
        for (AstArray<char> string : expr->strings)
        {
            formatCapacity += string.size + std::count(string.data, string.data + string.size, '%');
        }

        size_t skippedSubExpr = 0;
        for (size_t index = 0; index < expr->expressions.size; ++index)
        {
            const Constant* c = constants.find(expr->expressions.data[index]);
            if (c && c->type == Constant::Type::Type_String)
            {
                formatCapacity += c->stringLength + std::count(c->valueString, c->valueString + c->stringLength, '%');
                skippedSubExpr++;
            }
            else
                formatCapacity += 2;
        }

        std::string formatString;
        formatString.reserve(formatCapacity);

        LUAU_ASSERT(expr->strings.size == expr->expressions.size + 1);
        for (size_t idx = 0; idx < expr->strings.size; idx++)
        {
            AstArray<char> string = expr->strings.data[idx];
            escapeAndAppend(formatString, string.data, string.size);

            if (idx < expr->expressions.size)
            {
                const Constant* c = constants.find(expr->expressions.data[idx]);
                if (c && c->type == Constant::Type::Type_String)
                    escapeAndAppend(formatString, c->valueString, c->stringLength);
                else
                    formatString += "%*";
            }
        }

        int32_t formatStringIndex = -1;

        if (FFlag::LuauCompileNoInterpStorage)
        {
            if (formatString.empty())
                formatStringIndex = bytecode.addConstantString({"", 0});
            else
                formatStringIndex = bytecode.addConstantString(sref(names.getOrAdd(formatString.c_str(), formatString.size())));
        }
        else
        {
            size_t formatStringSize = formatString.size();

            std::unique_ptr<char[]> formatStringPtr(new char[formatStringSize]);
            memcpy(formatStringPtr.get(), formatString.data(), formatStringSize);

            AstArray<char> formatStringArray{formatStringPtr.get(), formatStringSize};
            interpStrings_DEPRECATED.emplace_back(std::move(formatStringPtr));

            formatStringIndex = bytecode.addConstantString(sref(formatStringArray));
        }

        if (formatStringIndex < 0)
            CompileError::raise(expr->location, "Exceeded constant limit; simplify the code to compile");

        RegScope rs(this);

        uint8_t baseReg = allocReg(expr, unsigned(2 + expr->expressions.size - skippedSubExpr));

        emitLoadK(baseReg, formatStringIndex);

        size_t skipped = 0;
        for (size_t index = 0; index < expr->expressions.size; ++index)
        {
            AstExpr* subExpr = expr->expressions.data[index];
            const Constant* c = constants.find(subExpr);
            if (!c || c->type != Constant::Type::Type_String)
                compileExprTempTop(subExpr, uint8_t(baseReg + 2 + index - skipped));
            else
                skipped++;
        }

        BytecodeBuilder::StringRef formatMethod = sref(AstName("format"));

        int32_t formatMethodIndex = bytecode.addConstantString(formatMethod);
        if (formatMethodIndex < 0)
            CompileError::raise(expr->location, "Exceeded constant limit; simplify the code to compile");

        bytecode.emitABC(LOP_NAMECALL, baseReg, baseReg, uint8_t(BytecodeBuilder::getStringHash(formatMethod)));
        bytecode.emitAux(formatMethodIndex);
        bytecode.emitABC(LOP_CALL, baseReg, uint8_t(expr->expressions.size + 2 - skippedSubExpr), 2);
        bytecode.emitABC(LOP_MOVE, target, baseReg, 0);
    }

    static uint8_t encodeHashSize(unsigned int hashSize)
    {
        size_t hashSizeLog2 = 0;
        while ((1u << hashSizeLog2) < hashSize)
            hashSizeLog2++;

        return hashSize == 0 ? 0 : uint8_t(hashSizeLog2 + 1);
    }

    void compileExprTable(AstExprTable* expr, uint8_t target, bool targetTemp)
    {
        if (expr->items.size == 0)
        {
            TableShape shape = tableShapes[expr];

            bytecode.addDebugRemark("allocation: table hash %d", shape.hashSize);

            bytecode.emitABC(LOP_NEWTABLE, target, encodeHashSize(shape.hashSize), 0);
            bytecode.emitAux(shape.arraySize);
            return;
        }

        unsigned int arraySize = 0;
        unsigned int hashSize = 0;
        unsigned int recordSize = 0;
        unsigned int indexSize = 0;

        for (size_t i = 0; i < expr->items.size; ++i)
        {
            const AstExprTable::Item& item = expr->items.data[i];

            arraySize += (item.kind == AstExprTable::Item::List);
            hashSize += (item.kind != AstExprTable::Item::List);
            recordSize += (item.kind == AstExprTable::Item::Record);
        }

        if (arraySize == 0 && hashSize > 0)
        {
            for (size_t i = 0; i < expr->items.size; ++i)
            {
                const AstExprTable::Item& item = expr->items.data[i];
                LUAU_ASSERT(item.key);

                const Constant* ckey = constants.find(item.key);

                indexSize += (ckey && ckey->type == Constant::Type_Number && ckey->valueNumber == double(indexSize + 1));
            }

            if (hashSize == recordSize + indexSize)
                hashSize = recordSize;
            else
                indexSize = 0;
        }

        int encodedHashSize = encodeHashSize(hashSize);

        RegScope rs(this);

        uint8_t reg = targetTemp ? target : allocReg(expr, 1u);

        if (arraySize == 0 && indexSize == 0 && hashSize == recordSize && recordSize >= 1 && recordSize <= BytecodeBuilder::TableShape::kMaxLength)
        {
            BytecodeBuilder::TableShape shape;

            for (size_t i = 0; i < expr->items.size; ++i)
            {
                const AstExprTable::Item& item = expr->items.data[i];
                LUAU_ASSERT(item.kind == AstExprTable::Item::Record);

                AstExprConstantString* ckey = item.key->as<AstExprConstantString>();
                LUAU_ASSERT(ckey);

                int cid = bytecode.addConstantString(sref(ckey->value));
                if (cid < 0)
                    CompileError::raise(ckey->location, "Exceeded constant limit; simplify the code to compile");

                LUAU_ASSERT(shape.length < BytecodeBuilder::TableShape::kMaxLength);

                shape.keys[shape.length++] = cid;
            }

            int32_t tid = bytecode.addConstantTable(shape);
            if (tid < 0)
                CompileError::raise(expr->location, "Exceeded constant limit; simplify the code to compile");

            bytecode.addDebugRemark("allocation: table template %d", hashSize);

            if (tid < 32768)
            {
                bytecode.emitAD(LOP_DUPTABLE, reg, int16_t(tid));
            }
            else
            {
                bytecode.emitABC(LOP_NEWTABLE, reg, uint8_t(encodedHashSize), 0);
                bytecode.emitAux(0);
            }
        }
        else
        {
            const AstExprTable::Item* last = expr->items.size > 0 ? &expr->items.data[expr->items.size - 1] : nullptr;

            bool trailingVarargs = last && last->kind == AstExprTable::Item::List && last->value->is<AstExprVarargs>();
            LUAU_ASSERT(!trailingVarargs || arraySize > 0);

            unsigned int arrayAllocation = arraySize - trailingVarargs + indexSize;

            if (hashSize == 0)
                bytecode.addDebugRemark("allocation: table array %d", arrayAllocation);
            else if (arrayAllocation == 0)
                bytecode.addDebugRemark("allocation: table hash %d", hashSize);
            else
                bytecode.addDebugRemark("allocation: table hash %d array %d", hashSize, arrayAllocation);

            bytecode.emitABC(LOP_NEWTABLE, reg, uint8_t(encodedHashSize), 0);
            bytecode.emitAux(arrayAllocation);
        }

        unsigned int arrayChunkSize = std::min(16u, arraySize);
        uint8_t arrayChunkReg = allocReg(expr, arrayChunkSize);
        unsigned int arrayChunkCurrent = 0;

        unsigned int arrayIndex = 1;
        bool multRet = false;

        for (size_t i = 0; i < expr->items.size; ++i)
        {
            const AstExprTable::Item& item = expr->items.data[i];

            AstExpr* key = item.key;
            AstExpr* value = item.value;

            setDebugLine(value);

            if (options.coverageLevel >= 2)
            {
                bytecode.emitABC(LOP_COVERAGE, 0, 0, 0);
            }

            if (arrayChunkCurrent > 0 && (key || arrayChunkCurrent == arrayChunkSize))
            {
                bytecode.emitABC(LOP_SETLIST, reg, arrayChunkReg, uint8_t(arrayChunkCurrent + 1));
                bytecode.emitAux(arrayIndex);
                arrayIndex += arrayChunkCurrent;
                arrayChunkCurrent = 0;
            }

            if (key)
            {
                RegScope rsi(this);

                LValue lv = compileLValueIndex(reg, key, rsi);
                uint8_t rv = compileExprAuto(value, rsi);

                compileAssign(lv, rv, nullptr);
            }
            else
            {
                uint8_t temp = uint8_t(arrayChunkReg + arrayChunkCurrent);

                if (i + 1 == expr->items.size)
                    multRet = compileExprTempMultRet(value, temp);
                else
                    compileExprTempTop(value, temp);

                arrayChunkCurrent++;
            }
        }

        if (arrayChunkCurrent)
        {
            bytecode.emitABC(LOP_SETLIST, reg, arrayChunkReg, multRet ? 0 : uint8_t(arrayChunkCurrent + 1));
            bytecode.emitAux(arrayIndex);
        }

        if (target != reg)
            bytecode.emitABC(LOP_MOVE, target, reg, 0);
    }

    bool canImport(AstExprGlobal* expr)
    {
        return options.optimizationLevel >= 1 && getGlobalState(globals, expr->name) != Global::Written;
    }

    bool canImportChain(AstExprGlobal* expr)
    {
        return options.optimizationLevel >= 1 && getGlobalState(globals, expr->name) == Global::Default;
    }

    void compileExprIndexName(AstExprIndexName* expr, uint8_t target)
    {
        setDebugLine(expr);

        AstExprGlobal* importRoot = 0;
        AstExprIndexName* import1 = 0;
        AstExprIndexName* import2 = 0;

        if (AstExprIndexName* index = expr->expr->as<AstExprIndexName>())
        {
            importRoot = index->expr->as<AstExprGlobal>();
            import1 = index;
            import2 = expr;
        }
        else
        {
            importRoot = expr->expr->as<AstExprGlobal>();
            import1 = expr;
        }

        if (importRoot && canImportChain(importRoot))
        {
            int32_t id0 = bytecode.addConstantString(sref(importRoot->name));
            int32_t id1 = bytecode.addConstantString(sref(import1->index));
            int32_t id2 = import2 ? bytecode.addConstantString(sref(import2->index)) : -1;

            if (id0 < 0 || id1 < 0 || (import2 && id2 < 0))
                CompileError::raise(expr->location, "Exceeded constant limit; simplify the code to compile");

            if (id0 < 1024 && id1 < 1024 && id2 < 1024)
            {
                uint32_t iid = import2 ? BytecodeBuilder::getImportId(id0, id1, id2) : BytecodeBuilder::getImportId(id0, id1);
                int32_t cid = bytecode.addImport(iid);

                if (cid >= 0 && cid < 32768)
                {
                    bytecode.emitAD(LOP_GETIMPORT, target, int16_t(cid));
                    bytecode.emitAux(iid);
                    return;
                }
            }
        }

        RegScope rs(this);
        uint8_t reg = compileExprAuto(expr->expr, rs);

        setDebugLine(expr->indexLocation);

        BytecodeBuilder::StringRef iname = sref(expr->index);
        int32_t cid = bytecode.addConstantString(iname);
        if (cid < 0)
            CompileError::raise(expr->location, "Exceeded constant limit; simplify the code to compile");

        bytecode.emitABC(LOP_GETTABLEKS, target, reg, uint8_t(BytecodeBuilder::getStringHash(iname)));
        bytecode.emitAux(cid);

        hintTemporaryExprRegType(expr->expr, reg, LBC_TYPE_TABLE, 2);
    }

    void compileExprIndexExpr(AstExprIndexExpr* expr, uint8_t target)
    {
        RegScope rs(this);

        Constant cv = getConstant(expr->index);

        if (cv.type == Constant::Type_Number && cv.valueNumber >= 1 && cv.valueNumber <= 256 && double(int(cv.valueNumber)) == cv.valueNumber)
        {
            uint8_t i = uint8_t(int(cv.valueNumber) - 1);

            uint8_t rt = compileExprAuto(expr->expr, rs);

            setDebugLine(expr->index);

            bytecode.emitABC(LOP_GETTABLEN, target, rt, i);

            if (FFlag::LuauCompileExtraTableHints)
                hintTemporaryExprRegType(expr->expr, rt, LBC_TYPE_TABLE, 1);
        }
        else if (cv.type == Constant::Type_String)
        {
            BytecodeBuilder::StringRef iname = sref(cv.getString());
            int32_t cid = bytecode.addConstantString(iname);
            if (cid < 0)
                CompileError::raise(expr->location, "Exceeded constant limit; simplify the code to compile");

            uint8_t rt = compileExprAuto(expr->expr, rs);

            setDebugLine(expr->index);

            bytecode.emitABC(LOP_GETTABLEKS, target, rt, uint8_t(BytecodeBuilder::getStringHash(iname)));
            bytecode.emitAux(cid);

            if (FFlag::LuauCompileExtraTableHints)
                hintTemporaryExprRegType(expr->expr, rt, LBC_TYPE_TABLE, 2);
        }
        else
        {
            uint8_t rt = compileExprAuto(expr->expr, rs);
            uint8_t ri = compileExprAuto(expr->index, rs);

            bytecode.emitABC(LOP_GETTABLE, target, rt, ri);

            if (FFlag::LuauCompileExtraTableHints)
            {
                hintTemporaryExprRegType(expr->expr, rt, LBC_TYPE_TABLE, 1);
                hintTemporaryExprRegType(expr->index, ri, LBC_TYPE_NUMBER, 1);
            }
        }
    }

    void compileExprGlobal(AstExprGlobal* expr, uint8_t target)
    {
        if (canImport(expr))
        {
            int32_t id0 = bytecode.addConstantString(sref(expr->name));
            if (id0 < 0)
                CompileError::raise(expr->location, "Exceeded constant limit; simplify the code to compile");

            if (id0 < 1024)
            {
                uint32_t iid = BytecodeBuilder::getImportId(id0);
                int32_t cid = bytecode.addImport(iid);

                if (cid >= 0 && cid < 32768)
                {
                    bytecode.emitAD(LOP_GETIMPORT, target, int16_t(cid));
                    bytecode.emitAux(iid);
                    return;
                }
            }
        }

        BytecodeBuilder::StringRef gname = sref(expr->name);
        int32_t cid = bytecode.addConstantString(gname);
        if (cid < 0)
            CompileError::raise(expr->location, "Exceeded constant limit; simplify the code to compile");

        bytecode.emitABC(LOP_GETGLOBAL, target, 0, uint8_t(BytecodeBuilder::getStringHash(gname)));
        bytecode.emitAux(cid);
    }

    void compileExprConstant(AstExpr* node, const Constant* cv, uint8_t target)
    {
        switch (cv->type)
        {
        case Constant::Type_Nil:
            bytecode.emitABC(LOP_LOADNIL, target, 0, 0);
            break;

        case Constant::Type_Boolean:
            bytecode.emitABC(LOP_LOADB, target, cv->valueBoolean, 0);
            break;

        case Constant::Type_Number:
        {
            double d = cv->valueNumber;

            if (d >= std::numeric_limits<int16_t>::min() && d <= std::numeric_limits<int16_t>::max() && double(int16_t(d)) == d &&
                !(d == 0.0 && signbit(d)))
            {
                bytecode.emitAD(LOP_LOADN, target, int16_t(d));
            }
            else
            {
                int32_t cid = bytecode.addConstantNumber(d);
                if (cid < 0)
                    CompileError::raise(node->location, "Exceeded constant limit; simplify the code to compile");

                emitLoadK(target, cid);
            }
        }
        break;

        case Constant::Type_Vector:
        {
            int32_t cid = bytecode.addConstantVector(cv->valueVector[0], cv->valueVector[1], cv->valueVector[2], cv->valueVector[3]);
            emitLoadK(target, cid);
        }
        break;

        case Constant::Type_String:
        {
            int32_t cid = bytecode.addConstantString(sref(cv->getString()));
            if (cid < 0)
                CompileError::raise(node->location, "Exceeded constant limit; simplify the code to compile");

            emitLoadK(target, cid);
        }
        break;

        default:
            LUAU_ASSERT(!"Unexpected constant type");
        }
    }

    void compileExpr(AstExpr* node, uint8_t target, bool targetTemp = false)
    {
        setDebugLine(node);

        if (options.coverageLevel >= 2 && needsCoverage(node))
        {
            bytecode.emitABC(LOP_COVERAGE, 0, 0, 0);
        }

        if (const Constant* cv = constants.find(node); cv && cv->type != Constant::Type_Unknown)
        {
            compileExprConstant(node, cv, target);
            return;
        }

        if (AstExprGroup* expr = node->as<AstExprGroup>())
        {
            compileExpr(expr->expr, target, targetTemp);
        }
        else if (node->is<AstExprConstantNil>())
        {
            bytecode.emitABC(LOP_LOADNIL, target, 0, 0);
        }
        else if (AstExprConstantBool* expr = node->as<AstExprConstantBool>())
        {
            bytecode.emitABC(LOP_LOADB, target, expr->value, 0);
        }
        else if (AstExprConstantNumber* expr = node->as<AstExprConstantNumber>())
        {
            int32_t cid = bytecode.addConstantNumber(expr->value);
            if (cid < 0)
                CompileError::raise(expr->location, "Exceeded constant limit; simplify the code to compile");

            emitLoadK(target, cid);
        }
        else if (AstExprConstantString* expr = node->as<AstExprConstantString>())
        {
            int32_t cid = bytecode.addConstantString(sref(expr->value));
            if (cid < 0)
                CompileError::raise(expr->location, "Exceeded constant limit; simplify the code to compile");

            emitLoadK(target, cid);
        }
        else if (AstExprLocal* expr = node->as<AstExprLocal>())
        {
            if (int reg = getExprLocalReg(expr); reg >= 0)
            {
                if (options.optimizationLevel == 0 || target != reg)
                    bytecode.emitABC(LOP_MOVE, target, uint8_t(reg), 0);
            }
            else
            {
                LUAU_ASSERT(expr->upvalue);
                uint8_t uid = getUpval(expr->local);

                bytecode.emitABC(LOP_GETUPVAL, target, uid, 0);
            }
        }
        else if (AstExprGlobal* expr = node->as<AstExprGlobal>())
        {
            compileExprGlobal(expr, target);
        }
        else if (AstExprVarargs* expr = node->as<AstExprVarargs>())
        {
            compileExprVarargs(expr, target, 1);
        }
        else if (AstExprCall* expr = node->as<AstExprCall>())
        {
            if (targetTemp && target == regTop - 1)
                compileExprCall(expr, target, 1, true);
            else
                compileExprCall(expr, target, 1);
        }
        else if (AstExprIndexName* expr = node->as<AstExprIndexName>())
        {
            compileExprIndexName(expr, target);
        }
        else if (AstExprIndexExpr* expr = node->as<AstExprIndexExpr>())
        {
            compileExprIndexExpr(expr, target);
        }
        else if (AstExprFunction* expr = node->as<AstExprFunction>())
        {
            compileExprFunction(expr, target);
        }
        else if (AstExprTable* expr = node->as<AstExprTable>())
        {
            compileExprTable(expr, target, targetTemp);
        }
        else if (AstExprUnary* expr = node->as<AstExprUnary>())
        {
            compileExprUnary(expr, target);
        }
        else if (AstExprBinary* expr = node->as<AstExprBinary>())
        {
            compileExprBinary(expr, target, targetTemp);
        }
        else if (AstExprTypeAssertion* expr = node->as<AstExprTypeAssertion>())
        {
            compileExpr(expr->expr, target, targetTemp);
        }
        else if (AstExprIfElse* expr = node->as<AstExprIfElse>())
        {
            compileExprIfElse(expr, target, targetTemp);
        }
        else if (AstExprInterpString* interpString = node->as<AstExprInterpString>())
        {
            compileExprInterpString(interpString, target, targetTemp);
        }
        else if (AstExprInstantiate* expr = node->as<AstExprInstantiate>())
        {
            LUAU_ASSERT(FFlag::LuauExplicitTypeInstantiationSyntax);
            compileExpr(expr->expr, target, targetTemp);
        }
        else
        {
            LUAU_ASSERT(!"Unknown expression type");
        }
    }

    void compileExprTemp(AstExpr* node, uint8_t target)
    {
        return compileExpr(node, target, true);
    }

    uint8_t compileExprAuto(AstExpr* node, RegScope&)
    {
        if (int reg = getExprLocalReg(node); reg >= 0)
            return uint8_t(reg);

        uint8_t reg = allocReg(node, 1u);

        compileExprTemp(node, reg);

        return reg;
    }

    void compileExprSide(AstExpr* node)
    {
        if (node->is<AstExprLocal>() || node->is<AstExprGlobal>() || node->is<AstExprVarargs>() || node->is<AstExprFunction>() || isConstant(node))
            return;

        if (!node->is<AstExprCall>())
            bytecode.addDebugRemark("expression only compiled for side effects");

        RegScope rsi(this);
        compileExprAuto(node, rsi);
    }

    void compileExprTempN(AstExpr* node, uint8_t target, uint8_t targetCount, bool targetTop)
    {
        LUAU_ASSERT(!targetTop || unsigned(target + targetCount) == regTop);

        if (targetCount == 255)
            CompileError::raise(node->location, "Exceeded result count limit; simplify the code to compile");

        if (AstExprCall* expr = node->as<AstExprCall>())
        {
            compileExprCall(expr, target, targetCount, targetTop);
        }
        else if (AstExprVarargs* expr = node->as<AstExprVarargs>())
        {
            compileExprVarargs(expr, target, targetCount);
        }
        else
        {
            compileExprTemp(node, target);

            for (size_t i = 1; i < targetCount; ++i)
                bytecode.emitABC(LOP_LOADNIL, uint8_t(target + i), 0, 0);
        }
    }

    void compileExprListTemp(const AstArray<AstExpr*>& list, uint8_t target, uint8_t targetCount, bool targetTop)
    {
        LUAU_ASSERT(!targetTop || unsigned(target + targetCount) == regTop);

        if (list.size == targetCount)
        {
            for (size_t i = 0; i < list.size; ++i)
                compileExprTemp(list.data[i], uint8_t(target + i));
        }
        else if (list.size > targetCount)
        {
            for (size_t i = 0; i < targetCount; ++i)
                compileExprTemp(list.data[i], uint8_t(target + i));

            for (size_t i = targetCount; i < list.size; ++i)
                compileExprSide(list.data[i]);
        }
        else if (list.size > 0)
        {
            for (size_t i = 0; i < list.size - 1; ++i)
                compileExprTemp(list.data[i], uint8_t(target + i));

            compileExprTempN(list.data[list.size - 1], uint8_t(target + list.size - 1), uint8_t(targetCount - (list.size - 1)), targetTop);
        }
        else
        {
            for (size_t i = 0; i < targetCount; ++i)
                bytecode.emitABC(LOP_LOADNIL, uint8_t(target + i), 0, 0);
        }
    }

    struct LValue
    {
        enum Kind
        {
            Kind_Local,
            Kind_Upvalue,
            Kind_Global,
            Kind_IndexName,
            Kind_IndexNumber,
            Kind_IndexExpr,
        };

        Kind kind;
        uint8_t reg;
        uint8_t upval;
        uint8_t index;
        uint8_t number;
        BytecodeBuilder::StringRef name;
        Location location;
    };

    LValue compileLValueIndex(uint8_t reg, AstExpr* index, RegScope& rs)
    {
        Constant cv = getConstant(index);

        if (cv.type == Constant::Type_Number && cv.valueNumber >= 1 && cv.valueNumber <= 256 && double(int(cv.valueNumber)) == cv.valueNumber)
        {
            LValue result = {LValue::Kind_IndexNumber};
            result.reg = reg;
            result.number = uint8_t(int(cv.valueNumber) - 1);
            result.location = index->location;

            return result;
        }
        else if (cv.type == Constant::Type_String)
        {
            LValue result = {LValue::Kind_IndexName};
            result.reg = reg;
            result.name = sref(cv.getString());
            result.location = index->location;

            return result;
        }
        else
        {
            LValue result = {LValue::Kind_IndexExpr};
            result.reg = reg;
            result.index = compileExprAuto(index, rs);
            result.location = index->location;

            return result;
        }
    }

    LValue compileLValue(AstExpr* node, RegScope& rs)
    {
        setDebugLine(node);

        if (AstExprLocal* expr = node->as<AstExprLocal>())
        {
            if (int reg = getExprLocalReg(expr); reg >= 0)
            {
                LValue result = {LValue::Kind_Local};
                result.reg = uint8_t(reg);
                result.location = node->location;

                return result;
            }
            else
            {
                LUAU_ASSERT(expr->upvalue);

                LValue result = {LValue::Kind_Upvalue};
                result.upval = getUpval(expr->local);
                result.location = node->location;

                return result;
            }
        }
        else if (AstExprGlobal* expr = node->as<AstExprGlobal>())
        {
            LValue result = {LValue::Kind_Global};
            result.name = sref(expr->name);
            result.location = node->location;

            return result;
        }
        else if (AstExprIndexName* expr = node->as<AstExprIndexName>())
        {
            LValue result = {LValue::Kind_IndexName};
            result.reg = compileExprAuto(expr->expr, rs);
            result.name = sref(expr->index);
            result.location = node->location;

            return result;
        }
        else if (AstExprIndexExpr* expr = node->as<AstExprIndexExpr>())
        {
            uint8_t reg = compileExprAuto(expr->expr, rs);

            return compileLValueIndex(reg, expr->index, rs);
        }
        else
        {
            LUAU_ASSERT(!"Unknown assignment expression");

            return LValue();
        }
    }

    void compileLValueUse(const LValue& lv, uint8_t reg, bool set, AstExpr* targetExpr)
    {
        setDebugLine(lv.location);

        switch (lv.kind)
        {
        case LValue::Kind_Local:
            if (set)
                bytecode.emitABC(LOP_MOVE, lv.reg, reg, 0);
            else
                bytecode.emitABC(LOP_MOVE, reg, lv.reg, 0);
            break;

        case LValue::Kind_Upvalue:
            bytecode.emitABC(set ? LOP_SETUPVAL : LOP_GETUPVAL, reg, lv.upval, 0);
            break;

        case LValue::Kind_Global:
        {
            int32_t cid = bytecode.addConstantString(lv.name);
            if (cid < 0)
                CompileError::raise(lv.location, "Exceeded constant limit; simplify the code to compile");

            bytecode.emitABC(set ? LOP_SETGLOBAL : LOP_GETGLOBAL, reg, 0, uint8_t(BytecodeBuilder::getStringHash(lv.name)));
            bytecode.emitAux(cid);
        }
        break;

        case LValue::Kind_IndexName:
        {
            int32_t cid = bytecode.addConstantString(lv.name);
            if (cid < 0)
                CompileError::raise(lv.location, "Exceeded constant limit; simplify the code to compile");

            bytecode.emitABC(set ? LOP_SETTABLEKS : LOP_GETTABLEKS, reg, lv.reg, uint8_t(BytecodeBuilder::getStringHash(lv.name)));
            bytecode.emitAux(cid);

            if (FFlag::LuauCompileExtraTableHints && targetExpr)
            {
                if (AstExprIndexName* targetExprIndexName = targetExpr->as<AstExprIndexName>())
                    hintTemporaryExprRegType(targetExprIndexName->expr, lv.reg, LBC_TYPE_TABLE, 2);
            }
        }
        break;

        case LValue::Kind_IndexNumber:
            bytecode.emitABC(set ? LOP_SETTABLEN : LOP_GETTABLEN, reg, lv.reg, lv.number);

            if (FFlag::LuauCompileExtraTableHints && targetExpr)
            {
                if (AstExprIndexExpr* targetExprIndexExpr = targetExpr->as<AstExprIndexExpr>())
                    hintTemporaryExprRegType(targetExprIndexExpr->expr, lv.reg, LBC_TYPE_TABLE, 1);
            }
            break;

        case LValue::Kind_IndexExpr:
            bytecode.emitABC(set ? LOP_SETTABLE : LOP_GETTABLE, reg, lv.reg, lv.index);

            if (FFlag::LuauCompileExtraTableHints && targetExpr)
            {
                if (AstExprIndexExpr* targetExprIndexExpr = targetExpr->as<AstExprIndexExpr>())
                {
                    hintTemporaryExprRegType(targetExprIndexExpr->expr, lv.reg, LBC_TYPE_TABLE, 1);
                    hintTemporaryExprRegType(targetExprIndexExpr->index, lv.index, LBC_TYPE_NUMBER, 1);
                }
            }
            break;

        default:
            LUAU_ASSERT(!"Unknown lvalue kind");
        }
    }

    void compileAssign(const LValue& lv, uint8_t source, AstExpr* targetExpr)
    {
        compileLValueUse(lv, source, true, targetExpr);
    }

    AstExprLocal* getExprLocal(AstExpr* node)
    {
        if (AstExprLocal* expr = node->as<AstExprLocal>())
            return expr;
        else if (AstExprGroup* expr = node->as<AstExprGroup>())
            return getExprLocal(expr->expr);
        else if (AstExprTypeAssertion* expr = node->as<AstExprTypeAssertion>())
            return getExprLocal(expr->expr);
        else
            return nullptr;
    }

    int getExprLocalReg(AstExpr* node)
    {
        if (AstExprLocal* expr = getExprLocal(node))
        {
            Local* l = locals.find(expr->local);

            return l && l->allocated ? l->reg : -1;
        }
        else
            return -1;
    }

    bool isStatBreak(AstStat* node)
    {
        if (AstStatBlock* stat = node->as<AstStatBlock>())
            return stat->body.size == 1 && stat->body.data[0]->is<AstStatBreak>();

        return node->is<AstStatBreak>();
    }

    AstStatContinue* extractStatContinue(AstStatBlock* block)
    {
        if (block->body.size == 1)
            return block->body.data[0]->as<AstStatContinue>();
        else
            return nullptr;
    }

    void compileStatIf(AstStatIf* stat)
    {
        if (isConstantFalse(stat->condition))
        {
            if (stat->elsebody)
                compileStat(stat->elsebody);
            return;
        }

        if (AstExprBinary* cand = stat->condition->as<AstExprBinary>(); cand && cand->op == AstExprBinary::And && isConstantFalse(cand->right))
        {
            compileExprSide(cand->left);
            if (stat->elsebody)
                compileStat(stat->elsebody);
            return;
        }

        if (!stat->elsebody && isStatBreak(stat->thenbody) && !areLocalsCaptured(loops.back().localOffset))
        {
            std::vector<size_t> elseJump;
            compileConditionValue(stat->condition, nullptr, elseJump, true);

            for (size_t jump : elseJump)
                loopJumps.push_back({LoopJump::Break, jump});
            return;
        }

        AstStatContinue* continueStatement = extractStatContinue(stat->thenbody);

        if (!stat->elsebody && continueStatement != nullptr && !areLocalsCaptured(loops.back().localOffsetContinue))
        {
            if (!loops.back().continueUsed)
                loops.back().continueUsed = continueStatement;

            std::vector<size_t> elseJump;
            compileConditionValue(stat->condition, nullptr, elseJump, true);

            for (size_t jump : elseJump)
                loopJumps.push_back({LoopJump::Continue, jump});
            return;
        }

        std::vector<size_t> elseJump;
        compileConditionValue(stat->condition, nullptr, elseJump, false);

        compileStat(stat->thenbody);

        if (stat->elsebody && elseJump.size() > 0)
        {
            if (alwaysTerminates(stat->thenbody))
            {
                size_t elseLabel = bytecode.emitLabel();

                compileStat(stat->elsebody);

                patchJumps(stat, elseJump, elseLabel);
            }
            else
            {
                size_t thenLabel = bytecode.emitLabel();

                bytecode.emitAD(LOP_JUMP, 0, 0);

                size_t elseLabel = bytecode.emitLabel();

                compileStat(stat->elsebody);

                size_t endLabel = bytecode.emitLabel();

                patchJumps(stat, elseJump, elseLabel);
                patchJump(stat, thenLabel, endLabel);
            }
        }
        else
        {
            size_t endLabel = bytecode.emitLabel();

            patchJumps(stat, elseJump, endLabel);
        }
    }

    void compileStatWhile(AstStatWhile* stat)
    {
        if (isConstantFalse(stat->condition))
            return;

        size_t oldJumps = loopJumps.size();
        size_t oldLocals = localStack.size();

        loops.push_back({oldLocals, oldLocals, nullptr});
        hasLoops = true;

        size_t loopLabel = bytecode.emitLabel();

        std::vector<size_t> elseJump;
        compileConditionValue(stat->condition, nullptr, elseJump, false);

        compileStat(stat->body);

        size_t contLabel = bytecode.emitLabel();

        size_t backLabel = bytecode.emitLabel();

        setDebugLine(stat->condition);

        bytecode.emitAD(LOP_JUMPBACK, 0, 0);

        size_t endLabel = bytecode.emitLabel();

        patchJump(stat, backLabel, loopLabel);
        patchJumps(stat, elseJump, endLabel);

        patchLoopJumps(stat, oldJumps, endLabel, contLabel);
        loopJumps.resize(oldJumps);

        loops.pop_back();
    }

    void compileStatRepeat(AstStatRepeat* stat)
    {
        size_t oldJumps = loopJumps.size();
        size_t oldLocals = localStack.size();

        loops.push_back({oldLocals, oldLocals, nullptr});
        hasLoops = true;

        size_t loopLabel = bytecode.emitLabel();

        AstStatBlock* body = stat->body;

        RegScope rs(this);

        bool continueValidated = false;
        size_t conditionLocals = 0;

        for (size_t i = 0; i < body->body.size; ++i)
        {
            compileStat(body->body.data[i]);

            loops.back().localOffsetContinue = localStack.size();

            if (loops.back().continueUsed && !continueValidated)
            {
                validateContinueUntil(loops.back().continueUsed, stat->condition, body, i + 1);
                continueValidated = true;
                conditionLocals = localStack.size();
            }
        }

        if (continueValidated)
        {
            setDebugLineEnd(body->body.data[body->body.size - 1]);

            closeLocals(conditionLocals);

            popLocals(conditionLocals);
        }

        size_t contLabel = bytecode.emitLabel();

        size_t endLabel;

        setDebugLine(stat->condition);

        if (isConstantTrue(stat->condition))
        {
            closeLocals(oldLocals);

            endLabel = bytecode.emitLabel();
        }
        else
        {
            std::vector<size_t> skipJump;
            compileConditionValue(stat->condition, nullptr, skipJump, true);

            closeLocals(oldLocals);

            size_t backLabel = bytecode.emitLabel();

            bytecode.emitAD(LOP_JUMPBACK, 0, 0);

            size_t skipLabel = bytecode.emitLabel();

            closeLocals(oldLocals);

            endLabel = bytecode.emitLabel();

            patchJump(stat, backLabel, loopLabel);
            patchJumps(stat, skipJump, skipLabel);
        }

        popLocals(oldLocals);

        patchLoopJumps(stat, oldJumps, endLabel, contLabel);
        loopJumps.resize(oldJumps);

        loops.pop_back();
    }

    void compileInlineReturn(AstStatReturn* stat, bool fallthrough)
    {
        setDebugLine(stat);

        InlineFrame frame = inlineFrames.back();

        compileExprListTemp(stat->list, frame.target, frame.targetCount, false);

        closeLocals(frame.localOffset);

        if (FFlag::LuauCompileCallCostModel)
        {
            size_t jumpLabel = bytecode.emitLabel();
            bytecode.emitAD(LOP_JUMP, 0, 0);

            inlineFrames.back().returnJumps.push_back(jumpLabel);
        }
        else
        {
            if (!fallthrough)
            {
                size_t jumpLabel = bytecode.emitLabel();
                bytecode.emitAD(LOP_JUMP, 0, 0);

                inlineFrames.back().returnJumps.push_back(jumpLabel);
            }
        }
    }

    void compileStatReturn(AstStatReturn* stat)
    {
        if (stat->list.size >= 255)
            CompileError::raise(stat->location, "Exceeded return count limit; simplify the code to compile");

        RegScope rs(this);

        uint8_t temp = 0;
        bool consecutive = false;
        bool multRet = false;

        if (int reg = stat->list.size > 0 ? getExprLocalReg(stat->list.data[0]) : -1; reg >= 0)
        {
            temp = uint8_t(reg);
            consecutive = true;

            for (size_t i = 1; i < stat->list.size; ++i)
                if (getExprLocalReg(stat->list.data[i]) != int(temp + i))
                {
                    consecutive = false;
                    break;
                }
        }

        if (!consecutive && stat->list.size > 0)
        {
            temp = allocReg(stat, unsigned(stat->list.size));

            for (size_t i = 0; i < stat->list.size; ++i)
                if (i + 1 == stat->list.size)
                    multRet = compileExprTempMultRet(stat->list.data[i], uint8_t(temp + i));
                else
                    compileExprTempTop(stat->list.data[i], uint8_t(temp + i));
        }

        closeLocals(0);

        bytecode.emitABC(LOP_RETURN, uint8_t(temp), multRet ? 0 : uint8_t(stat->list.size + 1), 0);
    }

    bool areLocalsRedundant(AstStatLocal* stat)
    {
        if (stat->values.size > stat->vars.size)
            return false;

        for (AstLocal* local : stat->vars)
        {
            Variable* v = variables.find(local);

            if (!v || !v->constant)
                return false;
        }

        return true;
    }

    void compileStatLocal(AstStatLocal* stat)
    {
        if (options.optimizationLevel >= 1 && options.debugLevel <= 1 && areLocalsRedundant(stat))
            return;

        if (options.optimizationLevel >= 1 && stat->vars.size == 1 && stat->values.size == 1)
        {
            if (AstExprLocal* re = getExprLocal(stat->values.data[0]))
            {
                Variable* lv = variables.find(stat->vars.data[0]);
                Variable* rv = variables.find(re->local);

                if (int reg = getExprLocalReg(re); reg >= 0 && (!lv || !lv->written) && (!rv || !rv->written))
                {
                    pushLocal(stat->vars.data[0], uint8_t(reg), kDefaultAllocPc);
                    return;
                }
            }
        }

        uint8_t vars = allocReg(stat, unsigned(stat->vars.size));
        uint32_t allocpc = bytecode.getDebugPC();

        compileExprListTemp(stat->values, vars, uint8_t(stat->vars.size), true);

        for (size_t i = 0; i < stat->vars.size; ++i)
            pushLocal(stat->vars.data[i], uint8_t(vars + i), allocpc);
    }

    bool tryCompileUnrolledFor(AstStatFor* stat, int thresholdBase, int thresholdMaxBoost)
    {
        Constant one = {Constant::Type_Number};
        one.valueNumber = 1.0;

        Constant fromc = getConstant(stat->from);
        Constant toc = getConstant(stat->to);
        Constant stepc = stat->step ? getConstant(stat->step) : one;

        int tripCount = (fromc.type == Constant::Type_Number && toc.type == Constant::Type_Number && stepc.type == Constant::Type_Number)
                            ? getTripCount(fromc.valueNumber, toc.valueNumber, stepc.valueNumber)
                            : -1;

        if (tripCount < 0)
        {
            bytecode.addDebugRemark("loop unroll failed: invalid iteration count");
            return false;
        }

        if (tripCount > thresholdBase)
        {
            bytecode.addDebugRemark("loop unroll failed: too many iterations (%d)", tripCount);
            return false;
        }

        if (Variable* lv = variables.find(stat->var); lv && lv->written)
        {
            bytecode.addDebugRemark("loop unroll failed: mutable loop variable");
            return false;
        }

        AstLocal* var = stat->var;
        uint64_t costModel = modelCost(stat->body, &var, 1, builtins, constants);

        bool varc = true;
        int unrolledCost = computeCost(costModel, &varc, 1) * tripCount;
        int baselineCost = (computeCost(costModel, nullptr, 0) + 1) * tripCount;
        int unrollProfit = (unrolledCost == 0) ? thresholdMaxBoost : std::min(thresholdMaxBoost, 100 * baselineCost / unrolledCost);

        int threshold = thresholdBase * unrollProfit / 100;

        if (unrolledCost > threshold)
        {
            bytecode.addDebugRemark(
                "loop unroll failed: too expensive (iterations %d, cost %d, profit %.2fx)", tripCount, unrolledCost, double(unrollProfit) / 100
            );
            return false;
        }

        bytecode.addDebugRemark("loop unroll succeeded (iterations %d, cost %d, profit %.2fx)", tripCount, unrolledCost, double(unrollProfit) / 100);

        compileUnrolledFor(stat, tripCount, fromc.valueNumber, stepc.valueNumber);
        return true;
    }

    void compileUnrolledFor(AstStatFor* stat, int tripCount, double from, double step)
    {
        AstLocal* var = stat->var;

        size_t oldLocals = localStack.size();
        size_t oldJumps = loopJumps.size();

        loops.push_back({oldLocals, oldLocals, nullptr});

        for (int iv = 0; iv < tripCount; ++iv)
        {
            locstants[var].type = Constant::Type_Number;
            locstants[var].valueNumber = from + iv * step;

            foldConstants(constants, variables, locstants, builtinsFold, builtinsFoldLibraryK, options.libraryMemberConstantCb, stat, names);

            size_t iterJumps = loopJumps.size();

            compileStat(stat->body);

            size_t contLabel = bytecode.emitLabel();

            for (size_t i = iterJumps; i < loopJumps.size(); ++i)
                if (loopJumps[i].type == LoopJump::Continue)
                    patchJump(stat, loopJumps[i].label, contLabel);
        }

        size_t endLabel = bytecode.emitLabel();

        for (size_t i = oldJumps; i < loopJumps.size(); ++i)
            if (loopJumps[i].type == LoopJump::Break)
                patchJump(stat, loopJumps[i].label, endLabel);

        loopJumps.resize(oldJumps);

        loops.pop_back();

        locstants[var].type = Constant::Type_Unknown;

        foldConstants(constants, variables, locstants, builtinsFold, builtinsFoldLibraryK, options.libraryMemberConstantCb, stat, names);
    }

    void compileStatFor(AstStatFor* stat)
    {
        RegScope rs(this);

        if (options.optimizationLevel >= 2 && isConstant(stat->to) && isConstant(stat->from) && (!stat->step || isConstant(stat->step)))
            if (tryCompileUnrolledFor(stat, FInt::LuauCompileLoopUnrollThreshold, FInt::LuauCompileLoopUnrollThresholdMaxBoost))
                return;

        size_t oldLocals = localStack.size();
        size_t oldJumps = loopJumps.size();

        loops.push_back({oldLocals, oldLocals, nullptr});
        hasLoops = true;

        uint8_t regs = allocReg(stat, 3u);

        uint8_t varreg = regs + 2;
        uint32_t varregallocpc = bytecode.getDebugPC();

        if (Variable* il = variables.find(stat->var); il && il->written)
            varreg = allocReg(stat, 1u);

        compileExprTemp(stat->from, uint8_t(regs + 2));
        compileExprTemp(stat->to, uint8_t(regs + 0));

        if (stat->step)
            compileExprTemp(stat->step, uint8_t(regs + 1));
        else
            bytecode.emitABC(LOP_LOADN, uint8_t(regs + 1), 1, 0);

        size_t forLabel = bytecode.emitLabel();

        bytecode.emitAD(LOP_FORNPREP, regs, 0);

        size_t loopLabel = bytecode.emitLabel();

        if (varreg != regs + 2)
            bytecode.emitABC(LOP_MOVE, varreg, regs + 2, 0);

        pushLocal(stat->var, varreg, varregallocpc);

        compileStat(stat->body);

        closeLocals(oldLocals);
        popLocals(oldLocals);

        setDebugLine(stat);

        size_t contLabel = bytecode.emitLabel();

        size_t backLabel = bytecode.emitLabel();

        bytecode.emitAD(LOP_FORNLOOP, regs, 0);

        size_t endLabel = bytecode.emitLabel();

        patchJump(stat, forLabel, endLabel);
        patchJump(stat, backLabel, loopLabel);

        patchLoopJumps(stat, oldJumps, endLabel, contLabel);
        loopJumps.resize(oldJumps);

        loops.pop_back();
    }

    void compileStatForIn(AstStatForIn* stat)
    {
        RegScope rs(this);

        size_t oldLocals = localStack.size();
        size_t oldJumps = loopJumps.size();

        loops.push_back({oldLocals, oldLocals, nullptr});
        hasLoops = true;

        uint8_t regs = allocReg(stat, 3u);

        compileExprListTemp(stat->values, regs, 3, true);

        uint8_t vars = allocReg(stat, std::max(unsigned(stat->vars.size), 2u));
        LUAU_ASSERT(vars == regs + 3);
        uint32_t varsallocpc = bytecode.getDebugPC();

        LuauOpcode skipOp = LOP_FORGPREP;

        if (options.optimizationLevel >= 1 && stat->vars.size <= 2)
        {
            if (stat->values.size == 1 && stat->values.data[0]->is<AstExprCall>())
            {
                Builtin builtin = getBuiltin(stat->values.data[0]->as<AstExprCall>()->func, globals, variables);

                if (builtin.isGlobal("ipairs"))
                    skipOp = LOP_FORGPREP_INEXT;
                else if (builtin.isGlobal("pairs"))
                    skipOp = LOP_FORGPREP_NEXT;
            }
            else if (stat->values.size == 2)
            {
                Builtin builtin = getBuiltin(stat->values.data[0], globals, variables);

                if (builtin.isGlobal("next"))
                    skipOp = LOP_FORGPREP_NEXT;
            }
        }

        size_t skipLabel = bytecode.emitLabel();

        bytecode.emitAD(skipOp, regs, 0);

        size_t loopLabel = bytecode.emitLabel();

        for (size_t i = 0; i < stat->vars.size; ++i)
            pushLocal(stat->vars.data[i], uint8_t(vars + i), varsallocpc);

        compileStat(stat->body);

        closeLocals(oldLocals);
        popLocals(oldLocals);

        setDebugLine(stat);

        size_t contLabel = bytecode.emitLabel();

        size_t backLabel = bytecode.emitLabel();

        bytecode.emitAD(LOP_FORGLOOP, regs, 0);
        bytecode.emitAux((skipOp == LOP_FORGPREP_INEXT ? 0x80000000 : 0) | uint32_t(stat->vars.size));

        size_t endLabel = bytecode.emitLabel();

        patchJump(stat, skipLabel, backLabel);
        patchJump(stat, backLabel, loopLabel);

        patchLoopJumps(stat, oldJumps, endLabel, contLabel);
        loopJumps.resize(oldJumps);

        loops.pop_back();
    }

    struct Assignment
    {
        LValue lvalue;

        uint8_t conflictReg = kInvalidReg;
        uint8_t valueReg = kInvalidReg;
    };

    void resolveAssignConflicts(AstStat* stat, std::vector<Assignment>& vars, const AstArray<AstExpr*>& values)
    {
        struct Visitor : AstVisitor
        {
            Compiler* self;

            std::bitset<256> conflict;
            std::bitset<256> assigned;

            Visitor(Compiler* self)
                : self(self)
            {
            }

            bool visit(AstExprLocal* node) override
            {
                int reg = self->getLocalReg(node->local);

                if (reg >= 0 && assigned[reg])
                    conflict[reg] = true;

                return true;
            }
        };

        Visitor visitor(this);

        for (size_t i = 0; i < vars.size(); ++i)
        {
            const LValue& li = vars[i].lvalue;

            if (li.kind == LValue::Kind_Local)
            {
                if (i < values.size)
                    values.data[i]->visit(&visitor);

                visitor.assigned[li.reg] = true;
            }
        }

        for (size_t i = 0; i < vars.size(); ++i)
        {
            const LValue& li = vars[i].lvalue;

            if (li.kind != LValue::Kind_Local && i < values.size)
                values.data[i]->visit(&visitor);
        }

        for (size_t i = vars.size(); i < values.size; ++i)
            values.data[i]->visit(&visitor);

        for (const Assignment& var : vars)
        {
            const LValue& li = var.lvalue;

            if ((li.kind == LValue::Kind_IndexName || li.kind == LValue::Kind_IndexNumber || li.kind == LValue::Kind_IndexExpr) &&
                visitor.assigned[li.reg])
                visitor.conflict[li.reg] = true;

            if (li.kind == LValue::Kind_IndexExpr && visitor.assigned[li.index])
                visitor.conflict[li.index] = true;
        }

        for (Assignment& var : vars)
        {
            const LValue& li = var.lvalue;

            if (li.kind == LValue::Kind_Local && visitor.conflict[li.reg])
                var.conflictReg = allocReg(stat, 1u);
        }
    }

    void compileStatAssign(AstStatAssign* stat)
    {
        RegScope rs(this);

        if (stat->vars.size == 1 && stat->values.size == 1)
        {
            LValue var = compileLValue(stat->vars.data[0], rs);

            if (var.kind == LValue::Kind_Local)
            {
                compileExpr(stat->values.data[0], var.reg);
            }
            else
            {
                uint8_t reg = compileExprAuto(stat->values.data[0], rs);

                setDebugLine(stat->vars.data[0]);

                if (FFlag::LuauCompileExtraTableHints)
                    compileAssign(var, reg, stat->vars.data[0]);
                else
                    compileAssign(var, reg, nullptr);
            }
            return;
        }

        std::vector<Assignment> vars(stat->vars.size);

        for (size_t i = 0; i < stat->vars.size; ++i)
            vars[i].lvalue = compileLValue(stat->vars.data[i], rs);

        resolveAssignConflicts(stat, vars, stat->values);

        for (size_t i = 0; i < stat->vars.size && i < stat->values.size; ++i)
        {
            AstExpr* value = stat->values.data[i];

            if (i + 1 == stat->values.size && stat->vars.size > stat->values.size)
            {
                unsigned rest = stat->vars.size - stat->values.size + 1;
                uint8_t temp = allocReg(stat, rest);

                compileExprTempN(value, temp, uint8_t(rest), true);

                for (size_t j = i; j < stat->vars.size; ++j)
                    vars[j].valueReg = uint8_t(temp + (j - i));
            }
            else
            {
                Assignment& var = vars[i];

                if (var.lvalue.kind == LValue::Kind_Local)
                {
                    var.valueReg = (var.conflictReg == kInvalidReg) ? var.lvalue.reg : var.conflictReg;

                    compileExpr(stat->values.data[i], var.valueReg);
                }
                else
                {
                    var.valueReg = compileExprAuto(stat->values.data[i], rs);
                }
            }
        }

        for (size_t i = stat->vars.size; i < stat->values.size; ++i)
            compileExprSide(stat->values.data[i]);

        size_t varPos = 0;
        for (const Assignment& var : vars)
        {
            LUAU_ASSERT(var.valueReg != kInvalidReg);

            if (var.lvalue.kind != LValue::Kind_Local)
            {
                setDebugLine(var.lvalue.location);

                if (FFlag::LuauCompileExtraTableHints && varPos < stat->vars.size)
                    compileAssign(var.lvalue, var.valueReg, stat->vars.data[varPos]);
                else
                    compileAssign(var.lvalue, var.valueReg, nullptr);
            }

            if (FFlag::LuauCompileExtraTableHints)
                varPos++;
        }

        for (const Assignment& var : vars)
        {
            if (var.lvalue.kind == LValue::Kind_Local && var.valueReg != var.lvalue.reg)
                bytecode.emitABC(LOP_MOVE, var.lvalue.reg, var.valueReg, 0);
        }
    }

    void compileStatCompoundAssign(AstStatCompoundAssign* stat)
    {
        RegScope rs(this);

        LValue var = compileLValue(stat->var, rs);

        uint8_t target = (var.kind == LValue::Kind_Local) ? var.reg : allocReg(stat, 1u);

        switch (stat->op)
        {
        case AstExprBinary::Add:
        case AstExprBinary::Sub:
        case AstExprBinary::Mul:
        case AstExprBinary::Div:
        case AstExprBinary::FloorDiv:
        case AstExprBinary::Mod:
        case AstExprBinary::Pow:
        {
            if (var.kind != LValue::Kind_Local)
                compileLValueUse(var, target, false, FFlag::LuauCompileExtraTableHints ? stat->var : nullptr);

            int32_t rc = getConstantNumber(stat->value);

            if (rc >= 0 && rc <= 255)
            {
                bytecode.emitABC(getBinaryOpArith(stat->op, true), target, target, uint8_t(rc));
            }
            else
            {
                uint8_t rr = compileExprAuto(stat->value, rs);

                bytecode.emitABC(getBinaryOpArith(stat->op), target, target, rr);

                if (var.kind != LValue::Kind_Local)
                    hintTemporaryRegType(stat->var, target, LBC_TYPE_NUMBER, 1);

                hintTemporaryExprRegType(stat->value, rr, LBC_TYPE_NUMBER, 1);
            }
        }
        break;

        case AstExprBinary::Concat:
        {
            std::vector<AstExpr*> args = {stat->value};

            unrollConcats(args);

            uint8_t regs = allocReg(stat, unsigned(1 + args.size()));

            compileLValueUse(var, regs, false, FFlag::LuauCompileExtraTableHints ? stat->var : nullptr);

            for (size_t i = 0; i < args.size(); ++i)
                compileExprTemp(args[i], uint8_t(regs + 1 + i));

            bytecode.emitABC(LOP_CONCAT, target, regs, uint8_t(regs + args.size()));
        }
        break;

        default:
            LUAU_ASSERT(!"Unexpected compound assignment operation");
        }

        if (var.kind != LValue::Kind_Local)
            compileAssign(var, target, FFlag::LuauCompileExtraTableHints ? stat->var : nullptr);
    }

    void compileStatFunction(AstStatFunction* stat)
    {
        if (int reg = getExprLocalReg(stat->name); reg >= 0)
        {
            compileExpr(stat->func, uint8_t(reg));
            return;
        }

        RegScope rs(this);
        uint8_t reg = allocReg(stat, 1u);

        compileExprTemp(stat->func, reg);

        LValue var = compileLValue(stat->name, rs);
        compileAssign(var, reg, FFlag::LuauCompileExtraTableHints ? stat->name : nullptr);
    }

    void compileStat(AstStat* node)
    {
        setDebugLine(node);

        if (options.coverageLevel >= 1 && needsCoverage(node))
        {
            bytecode.emitABC(LOP_COVERAGE, 0, 0, 0);
        }

        if (AstStatBlock* stat = node->as<AstStatBlock>())
        {
            RegScope rs(this);

            size_t oldLocals = localStack.size();

            if (FFlag::LuauCompileCallCostModel)
            {
                for (size_t i = 0; i < stat->body.size; ++i)
                {
                    AstStat* bodyStat = stat->body.data[i];
                    compileStat(bodyStat);

                    if (alwaysTerminates(bodyStat))
                        break;
                }
            }
            else
            {
                for (size_t i = 0; i < stat->body.size; ++i)
                    compileStat(stat->body.data[i]);
            }

            closeLocals(oldLocals);

            popLocals(oldLocals);
        }
        else if (AstStatIf* stat = node->as<AstStatIf>())
        {
            compileStatIf(stat);
        }
        else if (AstStatWhile* stat = node->as<AstStatWhile>())
        {
            compileStatWhile(stat);
        }
        else if (AstStatRepeat* stat = node->as<AstStatRepeat>())
        {
            compileStatRepeat(stat);
        }
        else if (node->is<AstStatBreak>())
        {
            LUAU_ASSERT(!loops.empty());

            closeLocals(loops.back().localOffset);

            size_t label = bytecode.emitLabel();

            bytecode.emitAD(LOP_JUMP, 0, 0);

            loopJumps.push_back({LoopJump::Break, label});
        }
        else if (AstStatContinue* stat = node->as<AstStatContinue>())
        {
            LUAU_ASSERT(!loops.empty());

            if (!loops.back().continueUsed)
                loops.back().continueUsed = stat;

            closeLocals(loops.back().localOffsetContinue);

            size_t label = bytecode.emitLabel();

            bytecode.emitAD(LOP_JUMP, 0, 0);

            loopJumps.push_back({LoopJump::Continue, label});
        }
        else if (AstStatReturn* stat = node->as<AstStatReturn>())
        {
            if (options.optimizationLevel >= 2 && !inlineFrames.empty())
                compileInlineReturn(stat, false);
            else
                compileStatReturn(stat);
        }
        else if (AstStatExpr* stat = node->as<AstStatExpr>())
        {
            if (AstExprCall* expr = stat->expr->as<AstExprCall>())
            {
                uint8_t target = uint8_t(regTop);

                compileExprCall(expr, target, 0);
            }
            else
            {
                compileExprSide(stat->expr);
            }
        }
        else if (AstStatLocal* stat = node->as<AstStatLocal>())
        {
            compileStatLocal(stat);
        }
        else if (AstStatFor* stat = node->as<AstStatFor>())
        {
            compileStatFor(stat);
        }
        else if (AstStatForIn* stat = node->as<AstStatForIn>())
        {
            compileStatForIn(stat);
        }
        else if (AstStatAssign* stat = node->as<AstStatAssign>())
        {
            compileStatAssign(stat);
        }
        else if (AstStatCompoundAssign* stat = node->as<AstStatCompoundAssign>())
        {
            compileStatCompoundAssign(stat);
        }
        else if (AstStatFunction* stat = node->as<AstStatFunction>())
        {
            compileStatFunction(stat);
        }
        else if (AstStatLocalFunction* stat = node->as<AstStatLocalFunction>())
        {
            uint8_t var = allocReg(stat, 1u);

            pushLocal(stat->name, var, kDefaultAllocPc);
            compileExprFunction(stat->func, var);

            Local& l = locals[stat->name];

            l.debugpc = bytecode.getDebugPC();
        }
        else if (node->is<AstStatTypeAlias>())
        {
        }
        else if (node->is<AstStatTypeFunction>())
        {
        }
        else
        {
            LUAU_ASSERT(!"Unknown statement type");
        }
    }

    void validateContinueUntil(AstStat* cont, AstExpr* condition, AstStatBlock* body, size_t start)
    {
        UndefinedLocalVisitor visitor(this);

        for (size_t i = start; i < body->body.size; ++i)
        {
            if (AstStatLocal* stat = body->body.data[i]->as<AstStatLocal>())
            {
                for (AstLocal* local : stat->vars)
                    visitor.locals.insert(local);
            }
            else if (AstStatLocalFunction* stat = body->body.data[i]->as<AstStatLocalFunction>())
            {
                visitor.locals.insert(stat->name);
            }
        }

        condition->visit(&visitor);

        if (visitor.undef)
            CompileError::raise(
                condition->location,
                "Local %s used in the repeat..until condition is undefined because continue statement on line %d jumps over it",
                visitor.undef->name.value,
                cont->location.begin.line + 1
            );
    }

    void gatherConstUpvals(AstExprFunction* func)
    {
        ConstUpvalueVisitor visitor(this);
        func->body->visit(&visitor);

        for (AstLocal* local : visitor.upvals)
            getUpval(local);
    }

    void pushLocal(AstLocal* local, uint8_t reg, uint32_t allocpc)
    {
        if (localStack.size() >= kMaxLocalCount)
            CompileError::raise(
                local->location, "Out of local registers when trying to allocate %s: exceeded limit %d", local->name.value, kMaxLocalCount
            );

        localStack.push_back(local);

        Local& l = locals[local];

        LUAU_ASSERT(!l.allocated);

        l.reg = reg;
        l.allocated = true;
        l.debugpc = bytecode.getDebugPC();
        l.allocpc = allocpc == kDefaultAllocPc ? l.debugpc : allocpc;
    }

    bool areLocalsCaptured(size_t start)
    {
        LUAU_ASSERT(start <= localStack.size());

        for (size_t i = start; i < localStack.size(); ++i)
        {
            Local* l = locals.find(localStack[i]);
            LUAU_ASSERT(l);

            if (l->captured)
                return true;
        }

        return false;
    }

    void closeLocals(size_t start)
    {
        LUAU_ASSERT(start <= localStack.size());

        bool captured = false;
        uint8_t captureReg = 255;

        for (size_t i = start; i < localStack.size(); ++i)
        {
            Local* l = locals.find(localStack[i]);
            LUAU_ASSERT(l);

            if (l->captured)
            {
                captured = true;
                captureReg = std::min(captureReg, l->reg);
            }
        }

        if (captured)
        {
            bytecode.emitABC(LOP_CLOSEUPVALS, captureReg, 0, 0);
        }
    }

    void popLocals(size_t start)
    {
        LUAU_ASSERT(start <= localStack.size());

        for (size_t i = start; i < localStack.size(); ++i)
        {
            Local* l = locals.find(localStack[i]);
            LUAU_ASSERT(l);
            LUAU_ASSERT(l->allocated);

            l->allocated = false;

            if (options.debugLevel >= 2)
            {
                uint32_t debugpc = bytecode.getDebugPC();

                bytecode.pushDebugLocal(sref(localStack[i]->name), l->reg, l->debugpc, debugpc);
            }

            if (options.typeInfoLevel >= 1 && i >= argCount)
            {
                uint32_t debugpc = bytecode.getDebugPC();
                LuauBytecodeType ty = LBC_TYPE_ANY;

                if (LuauBytecodeType* recordedTy = localTypes.find(localStack[i]))
                    ty = *recordedTy;

                bytecode.pushLocalTypeInfo(ty, l->reg, l->allocpc, debugpc);
            }
        }

        localStack.resize(start);
    }

    void patchJump(AstNode* node, size_t label, size_t target)
    {
        if (!bytecode.patchJumpD(label, target))
            CompileError::raise(node->location, "Exceeded jump distance limit; simplify the code to compile");
    }

    void patchJumps(AstNode* node, std::vector<size_t>& labels, size_t target)
    {
        for (size_t l : labels)
            patchJump(node, l, target);
    }

    void patchLoopJumps(AstNode* node, size_t oldJumps, size_t endLabel, size_t contLabel)
    {
        LUAU_ASSERT(oldJumps <= loopJumps.size());

        for (size_t i = oldJumps; i < loopJumps.size(); ++i)
        {
            const LoopJump& lj = loopJumps[i];

            switch (lj.type)
            {
            case LoopJump::Break:
                patchJump(node, lj.label, endLabel);
                break;

            case LoopJump::Continue:
                patchJump(node, lj.label, contLabel);
                break;

            default:
                LUAU_ASSERT(!"Unknown loop jump type");
            }
        }
    }

    uint8_t allocReg(AstNode* node, unsigned int count)
    {
        unsigned int top = regTop;
        if (top + count > kMaxRegisterCount)
            CompileError::raise(node->location, "Out of registers when trying to allocate %d registers: exceeded limit %d", count, kMaxRegisterCount);

        regTop += count;
        stackSize = std::max(stackSize, regTop);

        return uint8_t(top);
    }

    template<typename T>
    uint8_t allocReg(AstNode* node, T count) = delete;

    void setDebugLine(AstNode* node)
    {
        if (options.debugLevel >= 1)
            bytecode.setDebugLine(node->location.begin.line + 1);
    }

    void setDebugLine(const Location& location)
    {
        if (options.debugLevel >= 1)
            bytecode.setDebugLine(location.begin.line + 1);
    }

    void setDebugLineEnd(AstNode* node)
    {
        if (options.debugLevel >= 1)
            bytecode.setDebugLine(node->location.end.line + 1);
    }

    bool needsCoverage(AstNode* node)
    {
        return !node->is<AstStatBlock>() && !node->is<AstStatTypeAlias>();
    }

    void hintTemporaryRegType(AstExpr* expr, int reg, LuauBytecodeType expectedType, int instLength)
    {
        if (LuauBytecodeType* ty = exprTypes.find(expr))
        {
            if (*ty != expectedType)
                bytecode.pushLocalTypeInfo(*ty, reg, bytecode.getDebugPC() - instLength, bytecode.getDebugPC());
        }
    }

    void hintTemporaryExprRegType(AstExpr* expr, int reg, LuauBytecodeType expectedType, int instLength)
    {
        if (!getExprLocal(expr))
            hintTemporaryRegType(expr, reg, expectedType, instLength);
    }

    struct FenvVisitor : AstVisitor
    {
        bool& getfenvUsed;
        bool& setfenvUsed;

        FenvVisitor(bool& getfenvUsed, bool& setfenvUsed)
            : getfenvUsed(getfenvUsed)
            , setfenvUsed(setfenvUsed)
        {
        }

        bool visit(AstExprGlobal* node) override
        {
            if (node->name == "getfenv")
                getfenvUsed = true;
            if (node->name == "setfenv")
                setfenvUsed = true;

            return false;
        }
    };

    struct FunctionVisitor : AstVisitor
    {
        std::vector<AstExprFunction*>& functions;
        bool hasTypes = false;
        bool hasNativeFunction = false;

        FunctionVisitor(std::vector<AstExprFunction*>& functions)
            : functions(functions)
        {
            functions.reserve(16);
        }

        bool visit(AstExprFunction* node) override
        {
            node->body->visit(this);

            for (AstLocal* arg : node->args)
                hasTypes |= arg->annotation != nullptr;

            functions.push_back(node);

            if (!hasNativeFunction && node->hasNativeAttribute())
                hasNativeFunction = true;

            return false;
        }

        bool visit(AstStatTypeFunction* node) override
        {
            return false;
        }
    };

    struct UndefinedLocalVisitor : AstVisitor
    {
        UndefinedLocalVisitor(Compiler* self)
            : self(self)
            , undef(nullptr)
            , locals(nullptr)
        {
        }

        void check(AstLocal* local)
        {
            if (!undef && locals.contains(local))
                undef = local;
        }

        bool visit(AstExprLocal* node) override
        {
            if (!node->upvalue)
                check(node->local);

            return false;
        }

        bool visit(AstExprFunction* node) override
        {
            const Function* f = self->functions.find(node);
            LUAU_ASSERT(f);

            for (AstLocal* uv : f->upvals)
            {
                LUAU_ASSERT(uv->functionDepth < node->functionDepth);

                if (uv->functionDepth == node->functionDepth - 1)
                    check(uv);
            }

            return false;
        }

        Compiler* self;
        AstLocal* undef;
        DenseHashSet<AstLocal*> locals;
    };

    struct ConstUpvalueVisitor : AstVisitor
    {
        ConstUpvalueVisitor(Compiler* self)
            : self(self)
        {
        }

        bool visit(AstExprLocal* node) override
        {
            if (node->upvalue && self->isConstant(node))
            {
                upvals.push_back(node->local);
            }

            return false;
        }

        bool visit(AstExprFunction* node) override
        {
            return false;
        }

        Compiler* self;
        std::vector<AstLocal*> upvals;
    };

    struct ReturnVisitor : AstVisitor
    {
        Compiler* self;
        bool returnsOne = true;

        ReturnVisitor(Compiler* self)
            : self(self)
        {
        }

        bool visit(AstExpr* expr) override
        {
            return false;
        }

        bool visit(AstStatReturn* stat) override
        {
            returnsOne &= stat->list.size == 1 && !self->isExprMultRet(stat->list.data[0]);

            return false;
        }
    };

    struct RegScope
    {
        RegScope(Compiler* self)
            : self(self)
            , oldTop(self->regTop)
        {
        }

        RegScope(Compiler* self, unsigned int top)
            : self(self)
            , oldTop(self->regTop)
        {
            LUAU_ASSERT(top <= self->regTop);
            self->regTop = top;
        }

        ~RegScope()
        {
            self->regTop = oldTop;
        }

        Compiler* self;
        unsigned int oldTop;
    };

    struct Function
    {
        uint32_t id;
        std::vector<AstLocal*> upvals;

        uint64_t costModel = 0;
        unsigned int stackSize = 0;
        bool canInline = false;
        bool returnsOne = false;
    };

    struct Local
    {
        uint8_t reg = 0;
        bool allocated = false;
        bool captured = false;
        uint32_t debugpc = 0;
        uint32_t allocpc = 0;
    };

    struct LoopJump
    {
        enum Type
        {
            Break,
            Continue
        };

        Type type;
        size_t label;
    };

    struct Loop
    {
        size_t localOffset;
        size_t localOffsetContinue;

        AstStatContinue* continueUsed;
    };

    struct InlineArg
    {
        AstLocal* local;

        uint8_t reg;
        Constant value;
        uint32_t allocpc;

        AstExpr* init;
    };

    struct InlineFrame
    {
        AstExprFunction* func;

        size_t localOffset;

        uint8_t target;
        uint8_t targetCount;

        std::vector<size_t> returnJumps;
    };

    struct Capture
    {
        LuauCaptureType type;
        uint8_t data;
    };

    BytecodeBuilder& bytecode;

    CompileOptions options;

    DenseHashMap<AstExprFunction*, Function> functions;
    DenseHashMap<AstLocal*, Local> locals;
    DenseHashMap<AstName, Global> globals;
    DenseHashMap<AstLocal*, Variable> variables;
    DenseHashMap<AstExpr*, Constant> constants;
    DenseHashMap<AstLocal*, Constant> locstants;
    DenseHashMap<AstExprTable*, TableShape> tableShapes;
    DenseHashMap<AstExprCall*, int> builtins;
    DenseHashMap<AstName, uint8_t> userdataTypes;
    DenseHashMap<AstExprFunction*, std::string> functionTypes;
    DenseHashMap<AstLocal*, LuauBytecodeType> localTypes;
    DenseHashMap<AstExpr*, LuauBytecodeType> exprTypes;

    DenseHashMap<AstExprCall*, int> inlineBuiltins{nullptr};
    DenseHashMap<AstExprCall*, int> inlineBuiltinsBackup{nullptr};

    BuiltinAstTypes builtinTypes;
    AstNameTable& names;

    const DenseHashMap<AstExprCall*, int>* builtinsFold = nullptr;
    bool builtinsFoldLibraryK = false;

    unsigned int regTop = 0;
    unsigned int stackSize = 0;
    size_t argCount = 0;
    bool hasLoops = false;

    bool getfenvUsed = false;
    bool setfenvUsed = false;

    std::vector<AstLocal*> localStack;
    std::vector<AstLocal*> upvals;
    std::vector<LoopJump> loopJumps;
    std::vector<Loop> loops;
    std::vector<InlineFrame> inlineFrames;
    std::vector<Capture> captures;
    std::vector<std::unique_ptr<char[]>> interpStrings_DEPRECATED;
};

static void setCompileOptionsForNativeCompilation(CompileOptions& options)
{
    options.optimizationLevel = 2;
    options.typeInfoLevel = 1;
}

void compileOrThrow(BytecodeBuilder& bytecode, const ParseResult& parseResult, AstNameTable& names, const CompileOptions& inputOptions)
{
    LUAU_TIMETRACE_SCOPE("compileOrThrow", "Compiler");

    LUAU_ASSERT(parseResult.root);
    LUAU_ASSERT(parseResult.errors.empty());

    CompileOptions options = inputOptions;
    uint8_t mainFlags = 0;

    for (const HotComment& hc : parseResult.hotcomments)
    {
        if (hc.header && hc.content.compare(0, 9, "optimize ") == 0)
            options.optimizationLevel = std::max(0, std::min(2, atoi(hc.content.c_str() + 9)));

        if (hc.header && hc.content == "native")
        {
            mainFlags |= LPF_NATIVE_MODULE;
            setCompileOptionsForNativeCompilation(options);
        }
    }

    AstStatBlock* root = parseResult.root;

    std::vector<AstExprFunction*> functions;
    Compiler::FunctionVisitor functionVisitor(functions);
    root->visit(&functionVisitor);

    if (functionVisitor.hasNativeFunction)
        setCompileOptionsForNativeCompilation(options);

    Compiler compiler(bytecode, options, names);

    assignMutable(compiler.globals, names, options.mutableGlobals);

    trackValues(compiler.globals, compiler.variables, root);

    if (options.optimizationLevel >= 1 && (names.get("getfenv").value || names.get("setfenv").value))
    {
        Compiler::FenvVisitor fenvVisitor(compiler.getfenvUsed, compiler.setfenvUsed);
        root->visit(&fenvVisitor);
    }

    if (options.optimizationLevel >= 2 && (!compiler.getfenvUsed && !compiler.setfenvUsed))
    {
        compiler.builtinsFold = &compiler.builtins;

        if (AstName math = names.get("math"); math.value && getGlobalState(compiler.globals, math) == Global::Default)
        {
            compiler.builtinsFoldLibraryK = true;
        }
        else if (const char* const* ptr = options.librariesWithKnownMembers)
        {
            for (; *ptr; ++ptr)
            {
                if (AstName name = names.get(*ptr); name.value && getGlobalState(compiler.globals, name) == Global::Default)
                {
                    compiler.builtinsFoldLibraryK = true;
                    break;
                }
            }
        }
    }

    if (options.optimizationLevel >= 1)
    {
        analyzeBuiltins(compiler.builtins, compiler.globals, compiler.variables, options, root, names);

        foldConstants(
            compiler.constants,
            compiler.variables,
            compiler.locstants,
            compiler.builtinsFold,
            compiler.builtinsFoldLibraryK,
            options.libraryMemberConstantCb,
            root,
            names
        );

        predictTableShapes(compiler.tableShapes, root);
    }

    if (const char* const* ptr = options.userdataTypes)
    {
        for (; *ptr; ++ptr)
        {
            if (AstName name = names.get(*ptr); name.value)
                compiler.userdataTypes[name] = bytecode.addUserdataType(name.value);
        }

        if (uintptr_t(ptr - options.userdataTypes) > (LBC_TYPE_TAGGED_USERDATA_END - LBC_TYPE_TAGGED_USERDATA_BASE))
            CompileError::raise(root->location, "Exceeded userdata type limit in the compilation options");
    }

    if (options.typeInfoLevel >= 1 || options.optimizationLevel >= 2)
        buildTypeMap(
            compiler.functionTypes,
            compiler.localTypes,
            compiler.exprTypes,
            root,
            options.vectorType,
            compiler.userdataTypes,
            compiler.builtinTypes,
            compiler.builtins,
            compiler.globals,
            options.libraryMemberTypeCb,
            bytecode
        );

    for (AstExprFunction* expr : functions)
    {
        uint8_t protoflags = 0;
        compiler.compileFunction(expr, protoflags);

        if ((protoflags & LPF_NATIVE_FUNCTION) && !(mainFlags & LPF_NATIVE_MODULE))
            mainFlags |= LPF_NATIVE_FUNCTION;
    }

    AstExprFunction main(
        root->location,
         AstArray<AstAttr*>({nullptr, 0}),
         AstArray<AstGenericType*>(),
         AstArray<AstGenericTypePack*>(),
         nullptr,
        AstArray<AstLocal*>(),
         true,
         Luau::Location(),
        root,
         0,
         AstName(),
         nullptr
    );
    uint32_t mainid = compiler.compileFunction(&main, mainFlags);

    const Compiler::Function* mainf = compiler.functions.find(&main);
    LUAU_ASSERT(mainf && mainf->upvals.empty());

    bytecode.setMainFunction(mainid);
    bytecode.finalize();
}

void compileOrThrow(BytecodeBuilder& bytecode, const std::string& source, const CompileOptions& options, const ParseOptions& parseOptions)
{
    Allocator allocator;
    AstNameTable names(allocator);
    ParseResult result = Parser::parse(source.c_str(), source.size(), names, allocator, parseOptions);

    if (!result.errors.empty())
#ifdef LUAU_NO_EXCEPTIONS
        abort();
#else
        throw ParseErrors(result.errors);
#endif

    compileOrThrow(bytecode, result, names, options);
}

std::string compile(const std::string& source, const CompileOptions& options, const ParseOptions& parseOptions, BytecodeEncoder* encoder)
{
    LUAU_TIMETRACE_SCOPE("compile", "Compiler");

    Allocator allocator;
    AstNameTable names(allocator);
    ParseResult result = Parser::parse(source.c_str(), source.size(), names, allocator, parseOptions);

    if (!result.errors.empty())
    {
        const Luau::ParseError& parseError = result.errors.front();
        std::string error = format(":%d: %s", parseError.getLocation().begin.line + 1, parseError.what());

        return BytecodeBuilder::getError(error);
    }

#ifdef LUAU_NO_EXCEPTIONS
    struct Call
    {
        BytecodeBuilder* bcb;
        const ParseResult* result;
        AstNameTable* names;
        const CompileOptions* options;
    };
    CompileErrorJump jump;
    BytecodeBuilder bcb(encoder);
    Call call{&bcb, &result, &names, &options};
    auto body = [](void* ctx, void*)
    {
        Call* c = static_cast<Call*>(ctx);
        compileOrThrow(*c->bcb, *c->result, *c->names, *c->options);
    };

    if (runCompileProtected(jump, body, &call, nullptr))
    {
        return bcb.getBytecode();
    }
    else
    {
        const CompileError& e = *jump.error;
        std::string error = format(":%d: %s", e.getLocation().begin.line + 1, e.what());
        return BytecodeBuilder::getError(error);
    }
#else
    try
    {
        BytecodeBuilder bcb(encoder);
        compileOrThrow(bcb, result, names, options);

        return bcb.getBytecode();
    }
    catch (CompileError& e)
    {
        std::string error = format(":%d: %s", e.getLocation().begin.line + 1, e.what());
        return BytecodeBuilder::getError(error);
    }
#endif
}

void setCompileConstantNil(CompileConstant* constant)
{
    Compile::Constant* target = reinterpret_cast<Compile::Constant*>(constant);

    target->type = Compile::Constant::Type_Nil;
}

void setCompileConstantBoolean(CompileConstant* constant, bool b)
{
    Compile::Constant* target = reinterpret_cast<Compile::Constant*>(constant);

    target->type = Compile::Constant::Type_Boolean;
    target->valueBoolean = b;
}

void setCompileConstantNumber(CompileConstant* constant, double n)
{
    Compile::Constant* target = reinterpret_cast<Compile::Constant*>(constant);

    target->type = Compile::Constant::Type_Number;
    target->valueNumber = n;
}

void setCompileConstantVector(CompileConstant* constant, float x, float y, float z, float w)
{
    Compile::Constant* target = reinterpret_cast<Compile::Constant*>(constant);

    target->type = Compile::Constant::Type_Vector;
    target->valueVector[0] = x;
    target->valueVector[1] = y;
    target->valueVector[2] = z;
    target->valueVector[3] = w;
}

void setCompileConstantString(CompileConstant* constant, const char* s, size_t l)
{
    Compile::Constant* target = reinterpret_cast<Compile::Constant*>(constant);

    if (l > std::numeric_limits<unsigned int>::max())
        CompileError::raise({}, "Exceeded custom string constant length limit");

    target->type = Compile::Constant::Type_String;
    target->stringLength = unsigned(l);
    target->valueString = s;
}

}
