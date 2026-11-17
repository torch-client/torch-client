#pragma once

#include "Luau/Allocator.h"
#include "Luau/Ast.h"
#include "Luau/Location.h"
#include "Luau/DenseHash.h"
#include "Luau/Common.h"

#include <vector>

namespace Luau
{

struct Lexeme
{
    enum Type
    {
        Eof = 0,

        Char_END = 256,

        Equal,
        LessEqual,
        GreaterEqual,
        NotEqual,
        Dot2,
        Dot3,
        SkinnyArrow,
        DoubleColon,
        FloorDiv,

        InterpStringBegin,
        InterpStringMid,
        InterpStringEnd,
        InterpStringSimple,

        AddAssign,
        SubAssign,
        MulAssign,
        DivAssign,
        FloorDivAssign,
        ModAssign,
        PowAssign,
        ConcatAssign,

        RawString,
        QuotedString,
        Number,
        Name,

        Comment,
        BlockComment,

        Attribute,
        AttributeOpen,

        BrokenString,
        BrokenComment,
        BrokenUnicode,
        BrokenInterpDoubleBrace,
        Error,

        Reserved_BEGIN,
        ReservedAnd = Reserved_BEGIN,
        ReservedBreak,
        ReservedDo,
        ReservedElse,
        ReservedElseif,
        ReservedEnd,
        ReservedFalse,
        ReservedFor,
        ReservedFunction,
        ReservedIf,
        ReservedIn,
        ReservedLocal,
        ReservedNil,
        ReservedNot,
        ReservedOr,
        ReservedRepeat,
        ReservedReturn,
        ReservedThen,
        ReservedTrue,
        ReservedUntil,
        ReservedWhile,
        Reserved_END
    };

    enum struct QuoteStyle
    {
        Single,
        Double,
    };

    Type type;
    Location location;

private:
    unsigned int length;

public:
    union
    {
        const char* data;
        const char* name;
        unsigned int codepoint;
    };

    Lexeme(const Location& location, Type type);
    Lexeme(const Location& location, char character);
    Lexeme(const Location& location, Type type, const char* data, size_t size);
    Lexeme(const Location& location, Type type, const char* name);

    unsigned int getLength() const;
    unsigned int getBlockDepth() const;
    QuoteStyle getQuoteStyle() const;

    std::string toString() const;
};

static_assert(sizeof(Lexeme) <= 32, "Size of `Lexeme` struct should be up to 32 bytes.");

class AstNameTable
{
public:
    AstNameTable(Allocator& allocator);

    AstName addStatic(const char* name, Lexeme::Type type = Lexeme::Name);

    std::pair<AstName, Lexeme::Type> getOrAddWithType(const char* name, size_t length);
    std::pair<AstName, Lexeme::Type> getWithType(const char* name, size_t length) const;

    AstName getOrAdd(const char* name, size_t len);
    AstName getOrAdd(const char* name);
    AstName get(const char* name) const;

private:
    struct Entry
    {
        AstName value;
        uint32_t length;
        Lexeme::Type type;

        bool operator==(const Entry& other) const;
    };

    struct EntryHash
    {
        size_t operator()(const Entry& e) const;
    };

    DenseHashSet<Entry, EntryHash> data;

    Allocator& allocator;
};

class Lexer
{
public:
    Lexer(const char* buffer, std::size_t bufferSize, AstNameTable& names, Position startPosition = {0, 0});

    void setSkipComments(bool skip);
    void setReadNames(bool read);

    const Location& previousLocation() const
    {
        return prevLocation;
    }

    const Lexeme& next();
    const Lexeme& next(bool skipComments, bool updatePrevLocation);
    void nextline();

    Lexeme lookahead();

    const Lexeme& current() const
    {
        return lexeme;
    }

    static bool isReserved(const std::string& word);

    static bool fixupQuotedString(std::string& data);
    static void fixupMultilineString(std::string& data);

    unsigned int getOffset() const
    {
        return offset;
    }

    enum class BraceType
    {
        InterpolatedString,
        Normal
    };

    std::optional<Lexer::BraceType> peekBraceStackTop();

private:
    char peekch() const;
    char peekch(unsigned int lookahead) const;

    Position position() const;

    void consume();
    void consumeAny();

    Lexeme readCommentBody();

    int skipLongSeparator();

    Lexeme readLongString(const Position& start, int sep, Lexeme::Type ok, Lexeme::Type broken);
    Lexeme readQuotedString();

    Lexeme readInterpolatedStringBegin();
    Lexeme readInterpolatedStringSection(Position start, Lexeme::Type formatType, Lexeme::Type endType);

    void readBackslashInString();

    std::pair<AstName, Lexeme::Type> readName();

    Lexeme readNumber(const Position& start, unsigned int startOffset);

    Lexeme readUtf8Error();
    Lexeme readNext();

    const char* buffer;
    std::size_t bufferSize;

    unsigned int offset;

    unsigned int line;
    unsigned int lineOffset;

    Lexeme lexeme;

    Location prevLocation;

    AstNameTable& names;

    bool skipComments;
    bool readNames;

    std::vector<BraceType> braceStack;
};

inline bool isSpace(char ch)
{
    return ch == ' ' || ch == '\t' || ch == '\r' || ch == '\n' || ch == '\v' || ch == '\f';
}

}
