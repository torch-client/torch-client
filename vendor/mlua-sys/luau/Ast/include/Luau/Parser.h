#pragma once

#include "Luau/Ast.h"
#include "Luau/Lexer.h"
#include "Luau/ParseOptions.h"
#include "Luau/ParseResult.h"
#include "Luau/StringUtils.h"
#include "Luau/DenseHash.h"
#include "Luau/Common.h"
#include "Luau/Cst.h"

#include <initializer_list>
#include <optional>
#include <tuple>

namespace Luau
{

template<typename T>
class TempVector
{
public:
    explicit TempVector(std::vector<T>& storage);

    ~TempVector();

    const T& operator[](std::size_t index) const;

    const T& front() const;

    const T& back() const;

    bool empty() const;

    std::size_t size() const;

    void push_back(const T& item);

    typename std::vector<T>::const_iterator begin() const
    {
        return storage.begin() + offset;
    }
    typename std::vector<T>::const_iterator end() const
    {
        return storage.begin() + offset + size_;
    }

private:
    std::vector<T>& storage;
    size_t offset;
    size_t size_;
};

class Parser
{
    template<typename Node, typename F>
    static ParseNodeResult<Node> runParse(
        const char* buffer,
        size_t bufferSize,
        AstNameTable& names,
        Allocator& allocator,
        ParseOptions options,
        F f
    );

public:
    static ParseResult parse(
        const char* buffer,
        std::size_t bufferSize,
        AstNameTable& names,
        Allocator& allocator,
        ParseOptions options = ParseOptions()
    );

    static ParseNodeResult<AstExpr> parseExpr(
        const char* buffer,
        std::size_t bufferSize,
        AstNameTable& names,
        Allocator& allocator,
        ParseOptions options = ParseOptions()
    );

    static ParseNodeResult<AstType> parseType(
        const char* buffer,
        std::size_t bufferSize,
        AstNameTable& names,
        Allocator& allocator,
        ParseOptions options = {}
    );

private:
    struct Name;
    struct Binding;

    Parser(const char* buffer, std::size_t bufferSize, AstNameTable& names, Allocator& allocator, const ParseOptions& options);

    bool blockFollow(const Lexeme& l);

    AstStatBlock* parseChunk();

    AstStatBlock* parseBlock();

    AstStatBlock* parseBlockNoScope();

    AstStat* parseStat();

    AstStat* parseIf();

    AstStat* parseWhile();

    AstStat* parseRepeat();

    AstStat* parseDo();

    AstStat* parseBreak();

    AstStat* parseContinue(const Location& start);

    AstStat* parseFor();

    AstExpr* parseFunctionName(bool& hasself, AstName& debugname);

    LUAU_FORCEINLINE AstStat* parseFunctionStat(const AstArray<AstAttr*>& attributes = {nullptr, 0});

    std::optional<AstAttr::Type> validateAttribute(
        Location loc,
        const char* attributeName,
        const TempVector<AstAttr*>& attributes,
        const AstArray<AstExpr*>& args
    );

    void parseAttribute(TempVector<AstAttr*>& attribute);

    AstArray<AstAttr*> parseAttributes();

    AstStat* parseAttributeStat();

    AstStat* parseLocal(const AstArray<AstAttr*>& attributes);

    AstStat* parseReturn();

    AstStat* parseTypeAlias(const Location& start, bool exported, Position typeKeywordPosition);

    AstStat* parseTypeFunction(const Location& start, bool exported, Position typeKeywordPosition);

    AstDeclaredExternTypeProperty parseDeclaredExternTypeMethod(const AstArray<AstAttr*>& attributes);
    AstDeclaredExternTypeProperty parseDeclaredExternTypeMethod_DEPRECATED();

    AstStat* parseDeclaration(const Location& start, const AstArray<AstAttr*>& attributes);

    AstStat* parseAssignment(AstExpr* initial);

    AstStat* parseCompoundAssignment(AstExpr* initial, AstExprBinary::Op op);

    std::pair<AstLocal*, AstArray<AstLocal*>> prepareFunctionArguments(const Location& start, bool hasself, const TempVector<Binding>& args);

    std::pair<AstExprFunction*, AstLocal*> parseFunctionBody(
        bool hasself,
        const Lexeme& matchFunction,
        const AstName& debugname,
        const Name* localName,
        const AstArray<AstAttr*>& attributes
    );

    void parseExprList(TempVector<AstExpr*>& result, TempVector<Position>* commaPositions = nullptr);

    Binding parseBinding();
    AstArray<Position> extractAnnotationColonPositions(const TempVector<Binding>& bindings);

    std::tuple<bool, Location, AstTypePack*> parseBindingList(
        TempVector<Binding>& result,
        bool allowDot3 = false,
        AstArray<Position>* commaPositions = nullptr,
        Position* initialCommaPosition = nullptr,
        Position* varargAnnotationColonPosition = nullptr
    );

    AstType* parseOptionalType();

    AstTypePack* parseTypeList(
        TempVector<AstType*>& result,
        TempVector<std::optional<AstArgumentName>>& resultNames,
        TempVector<Position>* commaPositions = nullptr,
        TempVector<std::optional<Position>>* nameColonPositions = nullptr
    );

    AstTypePack* parseOptionalReturnType(Position* returnSpecifierPosition = nullptr);
    AstTypePack* parseReturnType();

    struct TableIndexerResult
    {
        AstTableIndexer* node;
        Position indexerOpenPosition;
        Position indexerClosePosition;
        Position colonPosition;
    };

    TableIndexerResult parseTableIndexer(AstTableAccess access, std::optional<Location> accessLocation, Lexeme begin);

    AstTypeOrPack parseFunctionType(bool allowPack, const AstArray<AstAttr*>& attributes);
    AstType* parseFunctionTypeTail(
        const Lexeme& begin,
        const AstArray<AstAttr*>& attributes,
        AstArray<AstGenericType*> generics,
        AstArray<AstGenericTypePack*> genericPacks,
        AstArray<AstType*> params,
        AstArray<std::optional<AstArgumentName>> paramNames,
        AstTypePack* varargAnnotation
    );

    AstType* parseTableType(bool inDeclarationContext = false);
    AstTypeOrPack parseSimpleType(bool allowPack, bool inDeclarationContext = false);

    AstTypeOrPack parseSimpleTypeOrPack();
    AstType* parseType(bool inDeclarationContext = false);

    AstTypePack* parseTypePack();
    AstTypePack* parseVariadicArgumentTypePack();

    AstType* parseTypeSuffix(AstType* type, const Location& begin);

    static std::optional<AstExprUnary::Op> parseUnaryOp(const Lexeme& l);
    static std::optional<AstExprBinary::Op> parseBinaryOp(const Lexeme& l);
    static std::optional<AstExprBinary::Op> parseCompoundOp(const Lexeme& l);

    struct BinaryOpPriority
    {
        unsigned char left, right;
    };

    std::optional<AstExprUnary::Op> checkUnaryConfusables();
    std::optional<AstExprBinary::Op> checkBinaryConfusables(const BinaryOpPriority binaryPriority[], unsigned int limit);

    AstExpr* parseExpr(unsigned int limit = 0);

    AstExpr* parseNameExpr(const char* context = nullptr);

    AstExpr* parsePrefixExpr();

    AstExpr* parsePrimaryExpr(bool asStatement);
    AstExpr* parseMethodCall(Position start, AstExpr* expr);

    AstExpr* parseAssertionExpr();

    AstExpr* parseSimpleExpr();

    std::tuple<AstArray<AstExpr*>, Location, Location> parseCallList(TempVector<Position>* commaPositions);
    AstExpr* parseFunctionArgs(AstExpr* func, bool self);

    std::optional<CstExprTable::Separator> tableSeparator();

    AstExpr* parseTableConstructor();

    AstExpr* parseIfElseExpr();

    AstExpr* parseInterpString();

    AstArray<AstTypeOrPack> parseTypeInstantiationExpr(CstTypeInstantiation* cstNodeOut = nullptr, Location* endLocationOut = nullptr);

    AstExpr* parseExplicitTypeInstantiationExpr(Position start, AstExpr& basedOnExpr);

    std::optional<Name> parseNameOpt(const char* context = nullptr);
    Name parseName(const char* context = nullptr);
    Name parseIndexName(const char* context, const Position& previous);

    std::pair<AstArray<AstGenericType*>, AstArray<AstGenericTypePack*>> parseGenericTypeList(
        bool withDefaultValues,
        Position* openPosition = nullptr,
        AstArray<Position>* commaPositions = nullptr,
        Position* closePosition = nullptr
    );

    AstArray<AstTypeOrPack> parseTypeParams(
        Position* openingPosition = nullptr,
        TempVector<Position>* commaPositions = nullptr,
        Position* closingPosition = nullptr
    );

    std::optional<AstArray<char>> parseCharArray(AstArray<char>* originalString = nullptr);
    AstExpr* parseString();
    AstExpr* parseNumber();

    AstLocal* pushLocal(const Binding& binding);

    unsigned int saveLocals();

    void restoreLocals(unsigned int offset);

    std::pair<CstExprConstantString::QuoteStyle, unsigned int> extractStringDetails();

    bool expectAndConsume(char value, const char* context = nullptr);
    bool expectAndConsume(Lexeme::Type type, const char* context = nullptr);
    bool expectAndConsumeFailWithLookahead(Lexeme::Type type, const char* context);
    void expectAndConsumeFail(Lexeme::Type type, const char* context);

    struct MatchLexeme
    {
        MatchLexeme(const Lexeme& l)
            : type(l.type)
            , position(l.location.begin)
        {
        }

        Lexeme::Type type;
        Position position;
    };

    bool expectMatchAndConsume(char value, const MatchLexeme& begin, bool searchForMissing = false);
    void expectMatchAndConsumeFail(Lexeme::Type type, const MatchLexeme& begin, const char* extra = nullptr);
    bool expectMatchAndConsumeRecover(char value, const MatchLexeme& begin, bool searchForMissing);

    bool expectMatchEndAndConsume(Lexeme::Type type, const MatchLexeme& begin);
    bool expectMatchEndAndConsumeFailWithLookahead(Lexeme::Type type, const MatchLexeme& begin);

    template<typename T>
    AstArray<T> copy(const T* data, std::size_t size);

    template<typename T>
    AstArray<T> copy(const TempVector<T>& data);

    template<typename T>
    AstArray<T> copy(std::initializer_list<T> data);

    AstArray<char> copy(const std::string& data);

    void incrementRecursionCounter(const char* context);

    void report(const Location& location, const char* format, va_list args);
    void report(const Location& location, const char* format, ...) LUAU_PRINTF_ATTR(3, 4);

    void reportNameError(const char* context);

    AstStatError* reportStatError(
        const Location& location,
        const AstArray<AstExpr*>& expressions,
        const AstArray<AstStat*>& statements,
        const char* format,
        ...
    ) LUAU_PRINTF_ATTR(5, 6);
    AstExprError* reportExprError(const Location& location, const AstArray<AstExpr*>& expressions, const char* format, ...) LUAU_PRINTF_ATTR(4, 5);
    AstTypeError* reportTypeError(const Location& location, const AstArray<AstType*>& types, const char* format, ...) LUAU_PRINTF_ATTR(4, 5);
    AstTypeError* reportMissingTypeError(
        const Location& parseErrorLocation,
        const Location& astErrorLocation,
        const char* format,
        ...
    ) LUAU_PRINTF_ATTR(4, 5);

    AstExpr* reportFunctionArgsError(AstExpr* func, bool self);
    void reportAmbiguousCallError();

    void nextLexeme();

    struct Function
    {
        bool vararg;
        unsigned int loopDepth;

        Function()
            : vararg(false)
            , loopDepth(0)
        {
        }
    };

    struct Local
    {
        AstLocal* local;
        unsigned int offset;

        Local()
            : local(nullptr)
            , offset(0)
        {
        }
    };

    struct Name
    {
        AstName name;
        Location location;

        Name(const AstName& name, const Location& location)
            : name(name)
            , location(location)
        {
        }
    };

    struct Binding
    {
        Name name;
        AstType* annotation;
        Position colonPosition;

        explicit Binding(const Name& name, AstType* annotation = nullptr, Position colonPosition = {0, 0})
            : name(name)
            , annotation(annotation)
            , colonPosition(colonPosition)
        {
        }
    };

    ParseOptions options;

    Lexer lexer;
    Allocator& allocator;

    std::vector<Comment> commentLocations;
    std::vector<HotComment> hotcomments;

    bool hotcommentHeader = true;

    unsigned int recursionCounter;

    AstName nameSelf;
    AstName nameNumber;
    AstName nameError;
    AstName nameNil;

    MatchLexeme endMismatchSuspect;

    std::vector<Function> functionStack;
    size_t typeFunctionDepth = 0;

    DenseHashMap<AstName, AstLocal*> localMap;
    std::vector<AstLocal*> localStack;

    std::vector<ParseError> parseErrors;

    std::vector<unsigned int> matchRecoveryStopOnToken;

    std::vector<AstAttr*> scratchAttr;
    std::vector<AstStat*> scratchStat;
    std::vector<AstArray<char>> scratchString;
    std::vector<AstArray<char>> scratchString2;
    std::vector<AstExpr*> scratchExpr;
    std::vector<AstExpr*> scratchExprAux;
    std::vector<AstName> scratchName;
    std::vector<AstName> scratchPackName;
    std::vector<Binding> scratchBinding;
    std::vector<AstLocal*> scratchLocal;
    std::vector<AstTableProp> scratchTableTypeProps;
    std::vector<CstTypeTable::Item> scratchCstTableTypeProps;
    std::vector<AstType*> scratchType;
    std::vector<AstTypeOrPack> scratchTypeOrPack;
    std::vector<AstDeclaredExternTypeProperty> scratchDeclaredClassProps;
    std::vector<AstExprTable::Item> scratchItem;
    std::vector<CstExprTable::Item> scratchCstItem;
    std::vector<AstArgumentName> scratchArgName;
    std::vector<AstGenericType*> scratchGenericTypes;
    std::vector<AstGenericTypePack*> scratchGenericTypePacks;
    std::vector<std::optional<AstArgumentName>> scratchOptArgName;
    std::vector<Position> scratchPosition;
    std::vector<std::optional<Position>> scratchOptPosition;
    std::string scratchData;

    CstNodeMap cstNodeMap;
};

}
