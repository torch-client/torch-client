#include "lmem.h"

#include "lstate.h"
#include "ldo.h"
#include "ldebug.h"

#include <string.h>

#ifndef __has_feature
#define __has_feature(x) 0
#endif

#if __has_feature(address_sanitizer) || defined(LUAU_ENABLE_ASAN)
#include <sanitizer/asan_interface.h>
#define ASAN_POISON_MEMORY_REGION(addr, size) __asan_poison_memory_region((addr), (size))
#define ASAN_UNPOISON_MEMORY_REGION(addr, size) __asan_unpoison_memory_region((addr), (size))
#else
#define ASAN_POISON_MEMORY_REGION(addr, size) (void)0
#define ASAN_UNPOISON_MEMORY_REGION(addr, size) (void)0
#endif

#if defined(__APPLE__)
#define ABISWITCH(x64, ms32, gcc32) (sizeof(void*) == 8 ? x64 : gcc32)
#elif defined(__i386__) && defined(__MINGW32__) && !defined(__MINGW64__)
#define ABISWITCH(x64, ms32, gcc32) (ms32)
#elif defined(__i386__) && !defined(_MSC_VER)
#define ABISWITCH(x64, ms32, gcc32) (gcc32)
#else
#define ABISWITCH(x64, ms32, gcc32) (sizeof(void*) == 8 ? x64 : ms32)
#endif

#if LUA_VECTOR_SIZE == 4
static_assert(sizeof(TValue) == ABISWITCH(24, 24, 24), "size mismatch for value");
static_assert(sizeof(LuaNode) == ABISWITCH(48, 48, 48), "size mismatch for table entry");
#else
static_assert(sizeof(TValue) == ABISWITCH(16, 16, 16), "size mismatch for value");
static_assert(sizeof(LuaNode) == ABISWITCH(32, 32, 32), "size mismatch for table entry");
#endif

static_assert(offsetof(TString, data) == ABISWITCH(24, 20, 20), "size mismatch for string header");
static_assert(sizeof(LuaTable) == ABISWITCH(48, 32, 32), "size mismatch for table header");
static_assert(offsetof(Buffer, data) == ABISWITCH(8, 8, 8), "size mismatch for buffer header");

static_assert(offsetof(Udata, data) == 16, "data must be at precise offset provide proper alignment");

const size_t kSizeClasses = LUA_SIZECLASSES;

const size_t kMaxSmallSize = 1024;

const size_t kMaxSmallSizeUsed = 1024;

const size_t kLargePageThreshold = 512;

const size_t kExternalAllocatorMetaDataReduction = 24;

const size_t kSmallPageSize = 16 * 1024 - kExternalAllocatorMetaDataReduction;
const size_t kLargePageSize = 32 * 1024 - kExternalAllocatorMetaDataReduction;

const size_t kBlockHeader = sizeof(double) > sizeof(void*) ? sizeof(double) : sizeof(void*);
const size_t kGCOLinkOffset = (sizeof(GCheader) + sizeof(void*) - 1) & ~(sizeof(void*) - 1);

struct SizeClassConfig
{
    int sizeOfClass[kSizeClasses];
    int8_t classForSize[kMaxSmallSize + 1];
    int classCount = 0;

    SizeClassConfig()
    {
        memset(sizeOfClass, 0, sizeof(sizeOfClass));
        memset(classForSize, -1, sizeof(classForSize));

        for (int size = 8; size < 64; size += 8)
            sizeOfClass[classCount++] = size;

        for (int size = 64; size < 256; size += 16)
            sizeOfClass[classCount++] = size;

        for (int size = 256; size < 512; size += 32)
            sizeOfClass[classCount++] = size;

        for (int size = 512; size <= 1024; size += 64)
            sizeOfClass[classCount++] = size;

        LUAU_ASSERT(size_t(classCount) <= kSizeClasses);

        for (int klass = 0; klass < classCount; ++klass)
            classForSize[sizeOfClass[klass]] = int8_t(klass);

        for (int size = kMaxSmallSize - 1; size >= 0; --size)
            if (classForSize[size] < 0)
                classForSize[size] = classForSize[size + 1];
    }
};

const SizeClassConfig kSizeClassConfig;

#define sizeclass(sz) (size_t((sz) - 1) < kMaxSmallSizeUsed ? kSizeClassConfig.classForSize[sz] : -1)

#define metadata(block) (*(void**)(block))
#define freegcolink(block) (*(void**)((char*)block + kGCOLinkOffset))

#if defined(LUAU_ASSERTENABLED)
#define debugpageset(x) (x)
#else
#define debugpageset(x) NULL
#endif

struct lua_Page
{
    lua_Page* prev;
    lua_Page* next;

    lua_Page* listprev;
    lua_Page* listnext;

    int pageSize;
    int blockSize;

    void* freeList;
    int freeNext;
    int busyBlocks;

    char padding[sizeof(void*) == 8 ? 8 : 12];

    char data[1];
};

static_assert(offsetof(lua_Page, data) % 16 == 0, "data must be 16 byte aligned to provide properly aligned allocation of userdata objects");

l_noret luaM_toobig(lua_State* L)
{
    luaG_runerror(L, "memory allocation error: block too big");
}

static lua_Page* newpage(lua_State* L, lua_Page** pageset, int pageSize, int blockSize, int blockCount)
{
    global_State* g = L->global;

    LUAU_ASSERT(pageSize - int(offsetof(lua_Page, data)) >= blockSize * blockCount);

    lua_Page* page = (lua_Page*)(*g->frealloc)(g->ud, NULL, 0, pageSize);
    if (!page)
        luaD_throw(L, LUA_ERRMEM);

    ASAN_POISON_MEMORY_REGION(page->data, blockSize * blockCount);

    page->prev = NULL;
    page->next = NULL;

    page->listprev = NULL;
    page->listnext = NULL;

    page->pageSize = pageSize;
    page->blockSize = blockSize;

    page->freeList = NULL;
    page->freeNext = (blockCount - 1) * blockSize;
    page->busyBlocks = 0;

    if (pageset)
    {
        page->listnext = *pageset;
        if (page->listnext)
            page->listnext->listprev = page;
        *pageset = page;
    }

    return page;
}

LUAU_NOINLINE static lua_Page* newclasspage(lua_State* L, lua_Page** freepageset, lua_Page** pageset, uint8_t sizeClass, bool storeMetadata)
{
    int sizeOfClass = kSizeClassConfig.sizeOfClass[sizeClass];
    int pageSize = sizeOfClass > int(kLargePageThreshold) ? kLargePageSize : kSmallPageSize;
    int blockSize = sizeOfClass + (storeMetadata ? kBlockHeader : 0);
    int blockCount = (pageSize - offsetof(lua_Page, data)) / blockSize;

    lua_Page* page = newpage(L, pageset, pageSize, blockSize, blockCount);

    LUAU_ASSERT(!freepageset[sizeClass]);
    freepageset[sizeClass] = page;

    return page;
}

static void freepage(lua_State* L, lua_Page** pageset, lua_Page* page)
{
    global_State* g = L->global;

    if (pageset)
    {
        if (page->listnext)
            page->listnext->listprev = page->listprev;

        if (page->listprev)
            page->listprev->listnext = page->listnext;
        else if (*pageset == page)
            *pageset = page->listnext;
    }

    (*g->frealloc)(g->ud, page, page->pageSize, 0);
}

static void freeclasspage(lua_State* L, lua_Page** freepageset, lua_Page** pageset, lua_Page* page, uint8_t sizeClass)
{
    if (page->next)
        page->next->prev = page->prev;

    if (page->prev)
        page->prev->next = page->next;
    else if (freepageset[sizeClass] == page)
        freepageset[sizeClass] = page->next;

    freepage(L, pageset, page);
}

static void* newblock(lua_State* L, int sizeClass)
{
    global_State* g = L->global;
    lua_Page* page = g->freepages[sizeClass];

    if (!page)
        page = newclasspage(L, g->freepages, debugpageset(&g->allpages), sizeClass, true);

    LUAU_ASSERT(!page->prev);
    LUAU_ASSERT(page->freeList || page->freeNext >= 0);
    LUAU_ASSERT(size_t(page->blockSize) == kSizeClassConfig.sizeOfClass[sizeClass] + kBlockHeader);

    void* block;

    if (page->freeNext >= 0)
    {
        block = &page->data + page->freeNext;
        ASAN_UNPOISON_MEMORY_REGION(block, page->blockSize);

        page->freeNext -= page->blockSize;
        page->busyBlocks++;
    }
    else
    {
        block = page->freeList;
        ASAN_UNPOISON_MEMORY_REGION(block, page->blockSize);

        page->freeList = metadata(block);
        page->busyBlocks++;
    }

    metadata(block) = page;

    if (!page->freeList && page->freeNext < 0)
    {
        g->freepages[sizeClass] = page->next;
        if (page->next)
            page->next->prev = NULL;
        page->next = NULL;
    }

    return (char*)block + kBlockHeader;
}

static void* newgcoblock(lua_State* L, int sizeClass)
{
    global_State* g = L->global;
    lua_Page* page = g->freegcopages[sizeClass];

    if (!page)
        page = newclasspage(L, g->freegcopages, &g->allgcopages, sizeClass, false);

    LUAU_ASSERT(!page->prev);
    LUAU_ASSERT(page->freeList || page->freeNext >= 0);
    LUAU_ASSERT(page->blockSize == kSizeClassConfig.sizeOfClass[sizeClass]);

    void* block;

    if (page->freeNext >= 0)
    {
        block = &page->data + page->freeNext;
        ASAN_UNPOISON_MEMORY_REGION(block, page->blockSize);

        page->freeNext -= page->blockSize;
        page->busyBlocks++;
    }
    else
    {
        block = page->freeList;
        ASAN_UNPOISON_MEMORY_REGION((char*)block + sizeof(GCheader), page->blockSize - sizeof(GCheader));

        page->freeList = freegcolink(block);
        page->busyBlocks++;
    }

    if (!page->freeList && page->freeNext < 0)
    {
        g->freegcopages[sizeClass] = page->next;
        if (page->next)
            page->next->prev = NULL;
        page->next = NULL;
    }

    return block;
}

static void freeblock(lua_State* L, int sizeClass, void* block)
{
    global_State* g = L->global;

    LUAU_ASSERT(block);
    block = (char*)block - kBlockHeader;

    lua_Page* page = (lua_Page*)metadata(block);
    LUAU_ASSERT(page && page->busyBlocks > 0);
    LUAU_ASSERT(size_t(page->blockSize) == kSizeClassConfig.sizeOfClass[sizeClass] + kBlockHeader);
    LUAU_ASSERT(block >= page->data && block < (char*)page + page->pageSize);

    if (!page->freeList && page->freeNext < 0)
    {
        LUAU_ASSERT(!page->prev);
        LUAU_ASSERT(!page->next);

        page->next = g->freepages[sizeClass];
        if (page->next)
            page->next->prev = page;
        g->freepages[sizeClass] = page;
    }

    metadata(block) = page->freeList;
    page->freeList = block;

    ASAN_POISON_MEMORY_REGION(block, page->blockSize);

    page->busyBlocks--;

    if (page->busyBlocks == 0)
        freeclasspage(L, g->freepages, debugpageset(&g->allpages), page, sizeClass);
}

static void freegcoblock(lua_State* L, int sizeClass, void* block, lua_Page* page)
{
    LUAU_ASSERT(page && page->busyBlocks > 0);
    LUAU_ASSERT(page->blockSize == kSizeClassConfig.sizeOfClass[sizeClass]);
    LUAU_ASSERT(block >= page->data && block < (char*)page + page->pageSize);

    global_State* g = L->global;

    if (!page->freeList && page->freeNext < 0)
    {
        LUAU_ASSERT(!page->prev);
        LUAU_ASSERT(!page->next);

        page->next = g->freegcopages[sizeClass];
        if (page->next)
            page->next->prev = page;
        g->freegcopages[sizeClass] = page;
    }

    freegcolink(block) = page->freeList;
    page->freeList = block;

    ASAN_POISON_MEMORY_REGION((char*)block + sizeof(GCheader), page->blockSize - sizeof(GCheader));

    page->busyBlocks--;

    if (page->busyBlocks == 0)
        freeclasspage(L, g->freegcopages, &g->allgcopages, page, sizeClass);
}

void* luaM_new_(lua_State* L, size_t nsize, uint8_t memcat)
{
    global_State* g = L->global;

    int nclass = sizeclass(nsize);

    void* block = nclass >= 0 ? newblock(L, nclass) : (*g->frealloc)(g->ud, NULL, 0, nsize);
    if (block == NULL && nsize > 0)
        luaD_throw(L, LUA_ERRMEM);

    g->totalbytes += nsize;
    g->memcatbytes[memcat] += nsize;

    if (LUAU_UNLIKELY(!!g->cb.onallocate))
    {
        g->cb.onallocate(L, 0, nsize);
    }

    return block;
}

GCObject* luaM_newgco_(lua_State* L, size_t nsize, uint8_t memcat)
{
    LUAU_ASSERT(nsize >= kGCOLinkOffset + sizeof(void*));

    global_State* g = L->global;

    int nclass = sizeclass(nsize);

    void* block = NULL;

    if (nclass >= 0)
    {
        block = newgcoblock(L, nclass);
    }
    else
    {
        lua_Page* page = newpage(L, &g->allgcopages, offsetof(lua_Page, data) + int(nsize), int(nsize), 1);

        block = &page->data;
        ASAN_UNPOISON_MEMORY_REGION(block, page->blockSize);

        page->freeNext -= page->blockSize;
        page->busyBlocks++;
    }

    if (block == NULL && nsize > 0)
        luaD_throw(L, LUA_ERRMEM);

    g->totalbytes += nsize;
    g->memcatbytes[memcat] += nsize;

    if (LUAU_UNLIKELY(!!g->cb.onallocate))
    {
        g->cb.onallocate(L, 0, nsize);
    }

    return (GCObject*)block;
}

void luaM_free_(lua_State* L, void* block, size_t osize, uint8_t memcat)
{
    global_State* g = L->global;
    LUAU_ASSERT((osize == 0) == (block == NULL));

    int oclass = sizeclass(osize);

    if (oclass >= 0)
        freeblock(L, oclass, block);
    else
        (*g->frealloc)(g->ud, block, osize, 0);

    g->totalbytes -= osize;
    g->memcatbytes[memcat] -= osize;
}

void luaM_freegco_(lua_State* L, GCObject* block, size_t osize, uint8_t memcat, lua_Page* page)
{
    global_State* g = L->global;
    LUAU_ASSERT((osize == 0) == (block == NULL));

    int oclass = sizeclass(osize);

    if (oclass >= 0)
    {
        block->gch.tt = LUA_TNIL;

        freegcoblock(L, oclass, block, page);
    }
    else
    {
        LUAU_ASSERT(page->busyBlocks == 1);
        LUAU_ASSERT(size_t(page->blockSize) == osize);
        LUAU_ASSERT((void*)block == page->data);

        freepage(L, &g->allgcopages, page);
    }

    g->totalbytes -= osize;
    g->memcatbytes[memcat] -= osize;
}

void* luaM_realloc_(lua_State* L, void* block, size_t osize, size_t nsize, uint8_t memcat)
{
    global_State* g = L->global;
    LUAU_ASSERT((osize == 0) == (block == NULL));

    int nclass = sizeclass(nsize);
    int oclass = sizeclass(osize);
    void* result;

    if (nclass >= 0 || oclass >= 0)
    {
        result = nclass >= 0 ? newblock(L, nclass) : (*g->frealloc)(g->ud, NULL, 0, nsize);
        if (result == NULL && nsize > 0)
            luaD_throw(L, LUA_ERRMEM);

        if (osize > 0 && nsize > 0)
            memcpy(result, block, osize < nsize ? osize : nsize);

        if (oclass >= 0)
            freeblock(L, oclass, block);
        else
            (*g->frealloc)(g->ud, block, osize, 0);
    }
    else
    {
        result = (*g->frealloc)(g->ud, block, osize, nsize);
        if (result == NULL && nsize > 0)
            luaD_throw(L, LUA_ERRMEM);
    }

    LUAU_ASSERT((nsize == 0) == (result == NULL));
    g->totalbytes = (g->totalbytes - osize) + nsize;
    g->memcatbytes[memcat] += nsize - osize;

    if (LUAU_UNLIKELY(!!g->cb.onallocate))
    {
        g->cb.onallocate(L, osize, nsize);
    }

    return result;
}

void luaM_getpagewalkinfo(lua_Page* page, char** start, char** end, int* busyBlocks, int* blockSize)
{
    int blockCount = (page->pageSize - offsetof(lua_Page, data)) / page->blockSize;

    LUAU_ASSERT(page->freeNext >= -page->blockSize && page->freeNext <= (blockCount - 1) * page->blockSize);

    char* data = page->data;

    *start = data + page->freeNext + page->blockSize;
    *end = data + blockCount * page->blockSize;
    *busyBlocks = page->busyBlocks;
    *blockSize = page->blockSize;
}

void luaM_getpageinfo(lua_Page* page, int* pageBlocks, int* busyBlocks, int* blockSize, int* pageSize)
{
    *pageBlocks = (page->pageSize - offsetof(lua_Page, data)) / page->blockSize;
    *busyBlocks = page->busyBlocks;
    *blockSize = page->blockSize;
    *pageSize = page->pageSize;
}

lua_Page* luaM_getnextpage(lua_Page* page)
{
    return page->listnext;
}

void luaM_visitpage(lua_Page* page, void* context, bool (*visitor)(void* context, lua_Page* page, GCObject* gco))
{
    char* start;
    char* end;
    int busyBlocks;
    int blockSize;
    luaM_getpagewalkinfo(page, &start, &end, &busyBlocks, &blockSize);

    for (char* pos = start; pos != end; pos += blockSize)
    {
        GCObject* gco = (GCObject*)pos;

        if (gco->gch.tt == LUA_TNIL)
            continue;

        if (visitor(context, page, gco))
        {
            LUAU_ASSERT(busyBlocks > 0);

            if (--busyBlocks == 0)
                break;
        }
    }
}

void luaM_visitgco(lua_State* L, void* context, bool (*visitor)(void* context, lua_Page* page, GCObject* gco))
{
    global_State* g = L->global;

    for (lua_Page* curr = g->allgcopages; curr;)
    {
        lua_Page* next = curr->listnext;

        luaM_visitpage(curr, context, visitor);

        curr = next;
    }
}
