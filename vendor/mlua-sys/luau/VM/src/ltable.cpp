#include "ltable.h"

#include "lstate.h"
#include "ldebug.h"
#include "lgc.h"
#include "lmem.h"
#include "lnumutils.h"

#include <string.h>

#define MAXBITS 26
#define MAXSIZE (1 << MAXBITS)

static_assert(offsetof(LuaNode, val) == 0, "Unexpected Node memory layout, pointer cast in gval2slot is incorrect");

static_assert(TKey{{NULL}, {0}, LUA_TDEADKEY, 0}.tt == LUA_TDEADKEY, "not enough bits for tt");
static_assert(TKey{{NULL}, {0}, LUA_TNIL, MAXSIZE - 1}.next == MAXSIZE - 1, "not enough bits for next");
static_assert(TKey{{NULL}, {0}, LUA_TNIL, -(MAXSIZE - 1)}.next == -(MAXSIZE - 1), "not enough bits for next");

const LuaNode luaH_dummynode = {
    {{NULL}, {0}, LUA_TNIL},
    {{NULL}, {0}, LUA_TNIL, 0}
};

#define dummynode (&luaH_dummynode)

#define hashpow2(t, n) (gnode(t, lmod((n), sizenode(t))))

#define hashstr(t, str) hashpow2(t, (str)->hash)
#define hashboolean(t, p) hashpow2(t, p)

static LuaNode* hashpointer(const LuaTable* t, const void* p)
{
    unsigned int h = unsigned(uintptr_t(p));

    h ^= h >> 16;
    h *= 0x85ebca6bu;
    h ^= h >> 13;
    h *= 0xc2b2ae35u;
    h ^= h >> 16;

    return hashpow2(t, h);
}

static LuaNode* hashnum(const LuaTable* t, double n)
{
    static_assert(sizeof(double) == sizeof(unsigned int) * 2, "expected a 8-byte double");
    unsigned int i[2];
    memcpy(i, &n, sizeof(i));

    uint32_t h1 = i[0];
    uint32_t h2 = i[1] & 0x7fffffff;

    const uint32_t m = 0x5bd1e995;

    h1 ^= h2 >> 18;
    h1 *= m;
    h2 ^= h1 >> 22;
    h2 *= m;
    h1 ^= h2 >> 17;
    h1 *= m;
    h2 ^= h1 >> 19;
    h2 *= m;

    return hashpow2(t, h2);
}

static LuaNode* hashvec(const LuaTable* t, const float* v)
{
    unsigned int i[LUA_VECTOR_SIZE];
    memcpy(i, v, sizeof(i));

    i[0] = (i[0] == 0x80000000) ? 0 : i[0];
    i[1] = (i[1] == 0x80000000) ? 0 : i[1];
    i[2] = (i[2] == 0x80000000) ? 0 : i[2];

    i[0] ^= i[0] >> 17;
    i[1] ^= i[1] >> 17;
    i[2] ^= i[2] >> 17;

    unsigned int h = (i[0] * 73856093) ^ (i[1] * 19349663) ^ (i[2] * 83492791);

#if LUA_VECTOR_SIZE == 4
    i[3] = (i[3] == 0x80000000) ? 0 : i[3];
    i[3] ^= i[3] >> 17;
    h ^= i[3] * 39916801;
#endif

    return hashpow2(t, h);
}

static LuaNode* mainposition(const LuaTable* t, const TValue* key)
{
    switch (ttype(key))
    {
    case LUA_TNUMBER:
        return hashnum(t, nvalue(key));
    case LUA_TVECTOR:
        return hashvec(t, vvalue(key));
    case LUA_TSTRING:
        return hashstr(t, tsvalue(key));
    case LUA_TBOOLEAN:
        return hashboolean(t, bvalue(key));
    case LUA_TLIGHTUSERDATA:
        return hashpointer(t, pvalue(key));
    default:
        return hashpointer(t, gcvalue(key));
    }
}

static int arrayindex(double key)
{
    int i;
    luai_num2int(i, key);

    return luai_numeq(cast_num(i), key) ? i : -1;
}

static int findindex(lua_State* L, LuaTable* t, StkId key)
{
    int i;
    if (ttisnil(key))
        return -1;
    i = ttisnumber(key) ? arrayindex(nvalue(key)) : -1;
    if (0 < i && i <= t->sizearray)
        return i - 1;
    else
    {
        LuaNode* n = mainposition(t, key);
        for (;;)
        {
            if (luaO_rawequalKey(gkey(n), key) || (ttype(gkey(n)) == LUA_TDEADKEY && iscollectable(key) && gcvalue(gkey(n)) == gcvalue(key)))
            {
                i = cast_int(n - gnode(t, 0));
                return i + t->sizearray;
            }
            if (gnext(n) == 0)
                break;
            n += gnext(n);
        }
        luaG_runerror(L, "invalid key to 'next'");
    }
}

int luaH_next(lua_State* L, LuaTable* t, StkId key)
{
    int i = findindex(L, t, key);
    for (i++; i < t->sizearray; i++)
    {
        if (!ttisnil(&t->array[i]))
        {
            setnvalue(key, cast_num(i + 1));
            setobj2s(L, key + 1, &t->array[i]);
            return 1;
        }
    }
    for (i -= t->sizearray; i < sizenode(t); i++)
    {
        if (!ttisnil(gval(gnode(t, i))))
        {
            getnodekey(L, key, gnode(t, i));
            setobj2s(L, key + 1, gval(gnode(t, i)));
            return 1;
        }
    }
    return 0;
}

#define maybesetaboundary(t, boundary) \
    { \
        if (t->aboundary <= 0) \
            t->aboundary = -int(boundary); \
    }

#define getaboundary(t) (t->aboundary < 0 ? -t->aboundary : t->sizearray)

static int computesizes(int nums[], int* narray)
{
    int i;
    int twotoi;
    int a = 0;
    int na = 0;
    int n = 0;
    for (i = 0, twotoi = 1; twotoi / 2 < *narray; i++, twotoi *= 2)
    {
        if (nums[i] > 0)
        {
            a += nums[i];
            if (a > twotoi / 2)
            {
                n = twotoi;
                na = a;
            }
        }
        if (a == *narray)
            break;
    }
    *narray = n;
    LUAU_ASSERT(*narray / 2 <= na && na <= *narray);
    return na;
}

static int countint(double key, int* nums)
{
    int k = arrayindex(key);
    if (0 < k && k <= MAXSIZE)
    {
        nums[ceillog2(k)]++;
        return 1;
    }
    else
        return 0;
}

static int numusearray(const LuaTable* t, int* nums)
{
    int lg;
    int ttlg;
    int ause = 0;
    int i = 1;
    for (lg = 0, ttlg = 1; lg <= MAXBITS; lg++, ttlg *= 2)
    {
        int lc = 0;
        int lim = ttlg;
        if (lim > t->sizearray)
        {
            lim = t->sizearray;
            if (i > lim)
                break;
        }
        for (; i <= lim; i++)
        {
            if (!ttisnil(&t->array[i - 1]))
                lc++;
        }
        nums[lg] += lc;
        ause += lc;
    }
    return ause;
}

static int numusehash(const LuaTable* t, int* nums, int* pnasize)
{
    int totaluse = 0;
    int ause = 0;
    int i = sizenode(t);
    while (i--)
    {
        LuaNode* n = &t->node[i];
        if (!ttisnil(gval(n)))
        {
            if (ttisnumber(gkey(n)))
                ause += countint(nvalue(gkey(n)), nums);
            totaluse++;
        }
    }
    *pnasize += ause;
    return totaluse;
}

static void setarrayvector(lua_State* L, LuaTable* t, int size)
{
    if (size > MAXSIZE)
        luaG_runerror(L, "table overflow");
    luaM_reallocarray(L, t->array, t->sizearray, size, TValue, t->memcat);
    TValue* array = t->array;
    for (int i = t->sizearray; i < size; i++)
        setnilvalue(&array[i]);
    t->sizearray = size;
}

static void setnodevector(lua_State* L, LuaTable* t, int size)
{
    int lsize;
    if (size == 0)
    {
        t->node = cast_to(LuaNode*, dummynode);
        lsize = 0;
    }
    else
    {
        int i;
        lsize = ceillog2(size);
        if (lsize > MAXBITS)
            luaG_runerror(L, "table overflow");
        size = twoto(lsize);
        t->node = luaM_newarray(L, size, LuaNode, t->memcat);
        for (i = 0; i < size; i++)
        {
            LuaNode* n = gnode(t, i);
            gnext(n) = 0;
            setnilvalue(gkey(n));
            setnilvalue(gval(n));
        }
    }
    t->lsizenode = cast_byte(lsize);
    t->nodemask8 = cast_byte((1 << lsize) - 1);
    t->lastfree = size;
}

static TValue* newkey(lua_State* L, LuaTable* t, const TValue* key);

static TValue* arrayornewkey(lua_State* L, LuaTable* t, const TValue* key)
{
    if (ttisnumber(key))
    {
        int k;
        double n = nvalue(key);
        luai_num2int(k, n);
        if (luai_numeq(cast_num(k), n) && unsigned(k) - 1 < unsigned(t->sizearray))
            return &t->array[k - 1];
    }

    return newkey(L, t, key);
}

static void resize(lua_State* L, LuaTable* t, int nasize, int nhsize)
{
    if (nasize > MAXSIZE || nhsize > MAXSIZE)
        luaG_runerror(L, "table overflow");
    int oldasize = t->sizearray;
    int oldhsize = t->lsizenode;
    LuaNode* nold = t->node;
    if (nasize > oldasize)
        setarrayvector(L, t, nasize);
    setnodevector(L, t, nhsize);
    LuaNode* nnew = t->node;
    if (nasize < oldasize)
    {
        t->sizearray = nasize;
        for (int i = nasize; i < oldasize; i++)
        {
            if (!ttisnil(&t->array[i]))
            {
                TValue ok;
                setnvalue(&ok, cast_num(i + 1));
                setobjt2t(L, newkey(L, t, &ok), &t->array[i]);
            }
        }
        luaM_reallocarray(L, t->array, oldasize, nasize, TValue, t->memcat);
    }
    TValue* anew = t->array;
    for (int i = twoto(oldhsize) - 1; i >= 0; i--)
    {
        LuaNode* old = nold + i;
        if (!ttisnil(gval(old)))
        {
            TValue ok;
            getnodekey(L, &ok, old);
            setobjt2t(L, arrayornewkey(L, t, &ok), gval(old));
        }
    }

    LUAU_ASSERT(nnew == t->node);
    LUAU_ASSERT(anew == t->array);

    if (nold != dummynode)
        luaM_freearray(L, nold, twoto(oldhsize), LuaNode, t->memcat);
}

static int adjustasize(LuaTable* t, int size, const TValue* ek)
{
    bool tbound = t->node != dummynode || size < t->sizearray;
    int ekindex = ek && ttisnumber(ek) ? arrayindex(nvalue(ek)) : -1;
    while (size + 1 == ekindex || (tbound && !ttisnil(luaH_getnum(t, size + 1))))
        size++;
    return size;
}

void luaH_resizearray(lua_State* L, LuaTable* t, int nasize)
{
    int nsize = (t->node == dummynode) ? 0 : sizenode(t);
    int asize = adjustasize(t, nasize, NULL);
    resize(L, t, asize, nsize);
}

void luaH_resizehash(lua_State* L, LuaTable* t, int nhsize)
{
    resize(L, t, t->sizearray, nhsize);
}

static void rehash(lua_State* L, LuaTable* t, const TValue* ek)
{
    int nums[MAXBITS + 1];
    for (int i = 0; i <= MAXBITS; i++)
        nums[i] = 0;
    int nasize = numusearray(t, nums);
    int totaluse = nasize;
    totaluse += numusehash(t, nums, &nasize);

    if (ttisnumber(ek))
        nasize += countint(nvalue(ek), nums);
    totaluse++;

    int na = computesizes(nums, &nasize);
    int nh = totaluse - na;

    int nadjusted = adjustasize(t, nasize, ek);

    int aextra = nadjusted - nasize;

    if (aextra != 0)
    {
        nh -= aextra;

        nasize = nadjusted + aextra;

        nasize = adjustasize(t, nasize, ek);
    }

    resize(L, t, nasize, nh);
}

LuaTable* luaH_new(lua_State* L, int narray, int nhash)
{
    LuaTable* t = luaM_newgco(L, LuaTable, sizeof(LuaTable), L->activememcat);
    luaC_init(L, t, LUA_TTABLE);
    t->metatable = NULL;
    t->tmcache = cast_byte(~0);
    t->array = NULL;
    t->sizearray = 0;
    t->lastfree = 0;
    t->lsizenode = 0;
    t->readonly = 0;
    t->safeenv = 0;
    t->nodemask8 = 0;
    t->node = cast_to(LuaNode*, dummynode);
    if (narray > 0)
        setarrayvector(L, t, narray);
    if (nhash > 0)
        setnodevector(L, t, nhash);
    return t;
}

void luaH_free(lua_State* L, LuaTable* t, lua_Page* page)
{
    if (t->node != dummynode)
        luaM_freearray(L, t->node, sizenode(t), LuaNode, t->memcat);
    if (t->array)
        luaM_freearray(L, t->array, t->sizearray, TValue, t->memcat);
    luaM_freegco(L, t, sizeof(LuaTable), t->memcat, page);
}

static LuaNode* getfreepos(LuaTable* t)
{
    while (t->lastfree > 0)
    {
        t->lastfree--;

        LuaNode* n = gnode(t, t->lastfree);
        if (ttisnil(gkey(n)))
            return n;
    }
    return NULL;
}

static TValue* newkey(lua_State* L, LuaTable* t, const TValue* key)
{
    if (ttisnumber(key) && nvalue(key) == t->sizearray + 1)
    {
        rehash(L, t, key);

        return arrayornewkey(L, t, key);
    }

    LuaNode* mp = mainposition(t, key);
    if (!ttisnil(gval(mp)) || mp == dummynode)
    {
        LuaNode* n = getfreepos(t);
        if (n == NULL)
        {
            rehash(L, t, key);

            return arrayornewkey(L, t, key);
        }
        LUAU_ASSERT(n != dummynode);
        TValue mk;
        getnodekey(L, &mk, mp);
        LuaNode* othern = mainposition(t, &mk);
        if (othern != mp)
        {
            while (othern + gnext(othern) != mp)
                othern += gnext(othern);
            gnext(othern) = cast_int(n - othern);
            *n = *mp;
            if (gnext(mp) != 0)
            {
                gnext(n) += cast_int(mp - n);
                gnext(mp) = 0;
            }
            setnilvalue(gval(mp));
        }
        else
        {
            if (gnext(mp) != 0)
                gnext(n) = cast_int((mp + gnext(mp)) - n);
            else
                LUAU_ASSERT(gnext(n) == 0);
            gnext(mp) = cast_int(n - mp);
            mp = n;
        }
    }
    setnodekey(L, mp, key);
    luaC_barriert(L, t, key);
    LUAU_ASSERT(ttisnil(gval(mp)));
    return gval(mp);
}

const TValue* luaH_getnum(LuaTable* t, int key)
{
    if (unsigned(key) - 1 < unsigned(t->sizearray))
        return &t->array[key - 1];
    else if (t->node != dummynode)
    {
        double nk = cast_num(key);
        LuaNode* n = hashnum(t, nk);
        for (;;)
        {
            if (ttisnumber(gkey(n)) && luai_numeq(nvalue(gkey(n)), nk))
                return gval(n);
            if (gnext(n) == 0)
                break;
            n += gnext(n);
        }
        return luaO_nilobject;
    }
    else
        return luaO_nilobject;
}

const TValue* luaH_getstr(LuaTable* t, TString* key)
{
    LuaNode* n = hashstr(t, key);
    for (;;)
    {
        if (ttisstring(gkey(n)) && tsvalue(gkey(n)) == key)
            return gval(n);
        if (gnext(n) == 0)
            break;
        n += gnext(n);
    }
    return luaO_nilobject;
}

const TValue* luaH_getp(LuaTable* t, void* key, int tag)
{
    LuaNode* n = hashpointer(t, key);
    for (;;)
    {
        const TKey* nk = gkey(n);
        if (ttislightuserdata(nk) && pvalue(nk) == key && lightuserdatatag(nk) == tag)
            return gval(n);
        if (gnext(n) == 0)
            break;
        n += gnext(n);
    }
    return luaO_nilobject;
}

const TValue* luaH_get(LuaTable* t, const TValue* key)
{
    switch (ttype(key))
    {
    case LUA_TNIL:
        return luaO_nilobject;
    case LUA_TSTRING:
        return luaH_getstr(t, tsvalue(key));
    case LUA_TNUMBER:
    {
        int k;
        double n = nvalue(key);
        luai_num2int(k, n);
        if (luai_numeq(cast_num(k), nvalue(key)))
            return luaH_getnum(t, k);
        LUAU_FALLTHROUGH;
    }
    default:
    {
        LuaNode* n = mainposition(t, key);
        for (;;)
        {
            if (luaO_rawequalKey(gkey(n), key))
                return gval(n);
            if (gnext(n) == 0)
                break;
            n += gnext(n);
        }
        return luaO_nilobject;
    }
    }
}

TValue* luaH_set(lua_State* L, LuaTable* t, const TValue* key)
{
    const TValue* p = luaH_get(t, key);
    invalidateTMcache(t);
    if (p != luaO_nilobject)
        return cast_to(TValue*, p);
    else
        return luaH_newkey(L, t, key);
}

TValue* luaH_newkey(lua_State* L, LuaTable* t, const TValue* key)
{
    if (ttisnil(key))
        luaG_runerror(L, "table index is nil");
    else if (ttisnumber(key) && luai_numisnan(nvalue(key)))
        luaG_runerror(L, "table index is NaN");
    else if (ttisvector(key) && luai_vecisnan(vvalue(key)))
        luaG_runerror(L, "table index contains NaN");
    return newkey(L, t, key);
}

TValue* luaH_setnum(lua_State* L, LuaTable* t, int key)
{
    if (unsigned(key) - 1 < unsigned(t->sizearray))
        return &t->array[key - 1];
    const TValue* p = luaH_getnum(t, key);
    if (p != luaO_nilobject)
        return cast_to(TValue*, p);
    else
    {
        TValue k;
        setnvalue(&k, cast_num(key));
        return newkey(L, t, &k);
    }
}

TValue* luaH_setstr(lua_State* L, LuaTable* t, TString* key)
{
    const TValue* p = luaH_getstr(t, key);
    invalidateTMcache(t);
    if (p != luaO_nilobject)
        return cast_to(TValue*, p);
    else
    {
        TValue k;
        setsvalue(L, &k, key);
        return newkey(L, t, &k);
    }
}

TValue* luaH_setp(lua_State* L, LuaTable* t, void* key, int tag)
{
    const TValue* p = luaH_getp(t, key, tag);
    if (p != luaO_nilobject)
        return cast_to(TValue*, p);
    else
    {
        TValue k;
        setpvalue(&k, key, tag);
        return newkey(L, t, &k);
    }
}

static int updateaboundary(LuaTable* t, int boundary)
{
    if (boundary < t->sizearray && ttisnil(&t->array[boundary - 1]))
    {
        if (boundary >= 2 && !ttisnil(&t->array[boundary - 2]))
        {
            maybesetaboundary(t, boundary - 1);
            return boundary - 1;
        }
    }
    else if (boundary + 1 < t->sizearray && !ttisnil(&t->array[boundary]) && ttisnil(&t->array[boundary + 1]))
    {
        maybesetaboundary(t, boundary + 1);
        return boundary + 1;
    }

    return 0;
}

int luaH_getn(LuaTable* t)
{
    int boundary = getaboundary(t);

    if (boundary > 0)
    {
        if (!ttisnil(&t->array[t->sizearray - 1]) && t->node == dummynode)
            return t->sizearray;
        if (boundary < t->sizearray && !ttisnil(&t->array[boundary - 1]) && ttisnil(&t->array[boundary]))
            return boundary;

        int foundboundary = updateaboundary(t, boundary);
        if (foundboundary > 0)
            return foundboundary;
    }

    int j = t->sizearray;

    if (j > 0 && ttisnil(&t->array[j - 1]))
    {
        TValue* base = t->array;
        int rest = j;
        while (int half = rest >> 1)
        {
            base = ttisnil(&base[half]) ? base : base + half;
            rest -= half;
        }
        int boundary = !ttisnil(base) + int(base - t->array);
        maybesetaboundary(t, boundary);
        return boundary;
    }
    else
    {
        LUAU_ASSERT(t->node == dummynode || ttisnil(luaH_getnum(t, j + 1)));
        return j;
    }
}

LuaTable* luaH_clone(lua_State* L, LuaTable* tt)
{
    LuaTable* t = luaM_newgco(L, LuaTable, sizeof(LuaTable), L->activememcat);
    luaC_init(L, t, LUA_TTABLE);
    t->metatable = tt->metatable;
    t->tmcache = tt->tmcache;
    t->array = NULL;
    t->sizearray = 0;
    t->lsizenode = 0;
    t->nodemask8 = 0;
    t->readonly = 0;
    t->safeenv = 0;
    t->node = cast_to(LuaNode*, dummynode);
    t->lastfree = 0;

    if (tt->sizearray)
    {
        t->array = luaM_newarray(L, tt->sizearray, TValue, t->memcat);
        maybesetaboundary(t, getaboundary(tt));
        t->sizearray = tt->sizearray;

        memcpy(t->array, tt->array, t->sizearray * sizeof(TValue));
    }

    if (tt->node != dummynode)
    {
        int size = 1 << tt->lsizenode;
        t->node = luaM_newarray(L, size, LuaNode, t->memcat);
        t->lsizenode = tt->lsizenode;
        t->nodemask8 = tt->nodemask8;
        memcpy(t->node, tt->node, size * sizeof(LuaNode));
        t->lastfree = tt->lastfree;
    }

    return t;
}

void luaH_clear(LuaTable* tt)
{
    for (int i = 0; i < tt->sizearray; ++i)
    {
        setnilvalue(&tt->array[i]);
    }

    maybesetaboundary(tt, 0);

    if (tt->node != dummynode)
    {
        int size = sizenode(tt);
        tt->lastfree = size;
        for (int i = 0; i < size; ++i)
        {
            LuaNode* n = gnode(tt, i);
            setnilvalue(gkey(n));
            setnilvalue(gval(n));
            gnext(n) = 0;
        }
    }

    tt->tmcache = cast_byte(~0);
}
