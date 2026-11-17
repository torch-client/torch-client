#include "ldo.h"

#include "lstring.h"
#include "lfunc.h"
#include "lgc.h"
#include "lmem.h"
#include "lvm.h"

#if LUA_USE_LONGJMP
#include <setjmp.h>
#include <stdlib.h>
#else
#include <stdexcept>
#endif

#include <string.h>

LUAU_FASTFLAGVARIABLE(LuauStacklessPcall)

#define MAX_STACK_SIZE (int(1024 / sizeof(TValue)) * 1024 * 1024)

#if LUA_USE_LONGJMP
struct lua_jmpbuf
{
    lua_jmpbuf* volatile prev;
    volatile int status;
    jmp_buf buf;
};

#if defined(__linux__) || defined(__APPLE__)
#define LUAU_SETJMP(buf) _setjmp(buf)
#define LUAU_LONGJMP(buf, code) _longjmp(buf, code)
#else
#define LUAU_SETJMP(buf) setjmp(buf)
#define LUAU_LONGJMP(buf, code) longjmp(buf, code)
#endif

int luaD_rawrunprotected(lua_State* L, Pfunc f, void* ud)
{
    lua_jmpbuf jb;
    jb.prev = L->global->errorjmp;
    jb.status = 0;
    L->global->errorjmp = &jb;

    if (LUAU_SETJMP(jb.buf) == 0)
        f(L, ud);

    L->global->errorjmp = jb.prev;
    return jb.status;
}

l_noret luaD_throw(lua_State* L, int errcode)
{
    if (lua_jmpbuf* jb = L->global->errorjmp)
    {
        jb->status = errcode;
        LUAU_LONGJMP(jb->buf, 1);
    }

    if (L->global->cb.panic)
        L->global->cb.panic(L, errcode);

    abort();
}
#else
class lua_exception : public std::exception
{
public:
    lua_exception(lua_State* L, int status)
        : L(L)
        , status(status)
    {
    }

    const char* what() const throw() override
    {
        if (status == LUA_ERRRUN)
            if (const char* str = lua_tostring(L, -1))
                return str;

        switch (status)
        {
        case LUA_ERRRUN:
            return "lua_exception: runtime error";
        case LUA_ERRSYNTAX:
            return "lua_exception: syntax error";
        case LUA_ERRMEM:
            return "lua_exception: " LUA_MEMERRMSG;
        case LUA_ERRERR:
            return "lua_exception: " LUA_ERRERRMSG;
        default:
            return "lua_exception: unexpected exception status";
        }
    }

    int getStatus() const
    {
        return status;
    }

    const lua_State* getThread() const
    {
        return L;
    }

private:
    lua_State* L;
    int status;
};

int luaD_rawrunprotected(lua_State* L, Pfunc f, void* ud)
{
    int status = 0;

    try
    {
        f(L, ud);
        return 0;
    }
    catch (lua_exception& e)
    {
        LUAU_ASSERT(e.getThread() == L);

        status = e.getStatus();
    }
    catch (std::exception& e)
    {
        try
        {
            luaG_pusherror(L, e.what());
            status = LUA_ERRRUN;
        }
        catch (std::exception&)
        {
            status = LUA_ERRMEM;
        }
    }

    return status;
}

l_noret luaD_throw(lua_State* L, int errcode)
{
    throw lua_exception(L, errcode);
}
#endif

static void correctstack(lua_State* L, TValue* oldstack)
{
    L->top = (L->top - oldstack) + L->stack;
    for (UpVal* up = L->openupval; up != NULL; up = up->u.open.threadnext)
        up->v = (up->v - oldstack) + L->stack;
    for (CallInfo* ci = L->base_ci; ci <= L->ci; ci++)
    {
        ci->top = (ci->top - oldstack) + L->stack;
        ci->base = (ci->base - oldstack) + L->stack;
        ci->func = (ci->func - oldstack) + L->stack;
    }
    L->base = (L->base - oldstack) + L->stack;
}

void luaD_reallocstack(lua_State* L, int newsize, int fornewci)
{
    if (newsize > MAX_STACK_SIZE)
    {
        if (fornewci)
        {
            CallInfo* cip = L->ci - 1;

            L->ci = cip;
            L->base = cip->base;
            L->top = cip->top;
        }

        luaD_throw(L, LUA_ERRMEM);
    }

    TValue* oldstack = L->stack;
    int realsize = newsize + EXTRA_STACK;
    LUAU_ASSERT(L->stack_last - L->stack == L->stacksize - EXTRA_STACK);
    luaM_reallocarray(L, L->stack, L->stacksize, realsize, TValue, L->memcat);
    TValue* newstack = L->stack;
    for (int i = L->stacksize; i < realsize; i++)
        setnilvalue(newstack + i);
    L->stacksize = realsize;
    L->stack_last = newstack + newsize;
    correctstack(L, oldstack);
}

void luaD_reallocCI(lua_State* L, int newsize)
{
    CallInfo* oldci = L->base_ci;
    luaM_reallocarray(L, L->base_ci, L->size_ci, newsize, CallInfo, L->memcat);
    L->size_ci = newsize;
    L->ci = (L->ci - oldci) + L->base_ci;
    L->end_ci = L->base_ci + L->size_ci - 1;
}

void luaD_growstack(lua_State* L, int n)
{
    luaD_reallocstack(L, getgrownstacksize(L, n), 0);
}

CallInfo* luaD_growCI(lua_State* L)
{
    const int hardlimit = LUAI_MAXCALLS + (LUAI_MAXCALLS >> 3);

    if (L->size_ci >= hardlimit)
        luaD_throw(L, LUA_ERRERR);

    int request = L->size_ci * 2;
    luaD_reallocCI(L, L->size_ci >= LUAI_MAXCALLS ? hardlimit : request < LUAI_MAXCALLS ? request : LUAI_MAXCALLS);

    if (L->size_ci > LUAI_MAXCALLS)
        luaG_runerror(L, "stack overflow");

    return ++L->ci;
}

void luaD_checkCstack(lua_State* L)
{
    const int hardlimit = LUAI_MAXCCALLS + (LUAI_MAXCCALLS >> 3);

    if (L->nCcalls == LUAI_MAXCCALLS)
        luaG_runerror(L, "C stack overflow");
    else if (L->nCcalls >= hardlimit)
        luaD_throw(L, LUA_ERRERR);
}

static void performcall(lua_State* L, StkId func, int nresults, bool preparereentry)
{
    if (luau_precall(L, func, nresults) == PCRLUA)
    {
        L->ci->flags |= LUA_CALLINFO_RETURN;

        bool oldactive = L->isactive;
        L->isactive = true;
        luaC_threadbarrier(L);

        if (FFlag::LuauStacklessPcall)
        {
            if (preparereentry)
                L->status = SCHEDULED_REENTRY;
            else
                luau_execute(L);
        }
        else
        {
            luau_execute(L);
        }

        if (!oldactive)
            L->isactive = false;
    }
}

void luaD_callint(lua_State* L, StkId func, int nresults, bool preparereentry)
{
    if (++L->nCcalls >= LUAI_MAXCCALLS)
        luaD_checkCstack(L);

    bool fromyieldableccall = false;

    if (L->ci != L->base_ci)
    {
        Closure* ccl = clvalue(L->ci->func);

        if (ccl->isC && ccl->c.cont)
        {
            fromyieldableccall = true;
            L->baseCcalls++;
        }
    }

    ptrdiff_t funcoffset = savestack(L, func);
    ptrdiff_t cioffset = saveci(L, L->ci);

    performcall(L, func, nresults, preparereentry);

    bool yielded = FFlag::LuauStacklessPcall ? isyielded(L) : L->status == LUA_YIELD || L->status == LUA_BREAK;

    if (fromyieldableccall)
    {
        L->baseCcalls--;

        if (yielded)
        {
            CallInfo* callerci = restoreci(L, cioffset);
            callerci->top = restorestack(L, funcoffset) + (nresults != LUA_MULTRET ? nresults : 0);
        }
    }

    if (nresults != LUA_MULTRET && !yielded)
        L->top = restorestack(L, funcoffset) + nresults;

    L->nCcalls--;
    luaC_checkGC(L);
}

void luaD_call(lua_State* L, StkId func, int nresults)
{
    luaD_callint(L, func, nresults, false);
}

void luaD_callny(lua_State* L, StkId func, int nresults)
{
    if (++L->nCcalls >= LUAI_MAXCCALLS)
        luaD_checkCstack(L);

    LUAU_ASSERT(L->nCcalls > L->baseCcalls);

    ptrdiff_t funcoffset = savestack(L, func);

    performcall(L, func, nresults, false);

    if (FFlag::LuauStacklessPcall)
        LUAU_ASSERT(!isyielded(L));
    else
        LUAU_ASSERT(L->status != LUA_YIELD && L->status != LUA_BREAK);

    if (nresults != LUA_MULTRET)
        L->top = restorestack(L, funcoffset) + nresults;

    L->nCcalls--;
    luaC_checkGC(L);
}

void luaD_seterrorobj(lua_State* L, int errcode, StkId oldtop)
{
    switch (errcode)
    {
    case LUA_ERRMEM:
    {
        setsvalue(L, oldtop, luaS_newliteral(L, LUA_MEMERRMSG));
        break;
    }
    case LUA_ERRERR:
    {
        setsvalue(L, oldtop, luaS_newliteral(L, LUA_ERRERRMSG));
        break;
    }
    case LUA_ERRSYNTAX:
    case LUA_ERRRUN:
    {
        setobj2s(L, oldtop, L->top - 1);
        break;
    }
    }
    L->top = oldtop + 1;
}

static void resume_continue(lua_State* L)
{
    while ((FFlag::LuauStacklessPcall ? L->status == LUA_OK || L->status == SCHEDULED_REENTRY : L->status == LUA_OK) && L->ci > L->base_ci)
    {
        LUAU_ASSERT(L->baseCcalls == L->nCcalls);

        if (FFlag::LuauStacklessPcall)
            L->status = LUA_OK;

        Closure* cl = curr_func(L);

        if (cl->isC)
        {
            LUAU_ASSERT(cl->c.cont);

            int n = cl->c.cont(L, 0);

            if (L->status == LUA_BREAK || L->status == LUA_YIELD)
                break;

            luau_poscall(L, L->top - n);
        }
        else
        {
            luau_execute(L);
        }
    }
}

static void resume(lua_State* L, void* ud)
{
    StkId firstArg = cast_to(StkId, ud);

    if (FFlag::LuauStacklessPcall)
    {
        if (L->status == LUA_OK)
        {
            LUAU_ASSERT(L->ci == L->base_ci && firstArg >= L->base);
            if (firstArg == L->base)
                luaG_runerror(L, "cannot resume dead coroutine");

            int precallresult = luau_precall(L, firstArg - 1, LUA_MULTRET);

            if (L->status == SCHEDULED_REENTRY)
            {
                firstArg = L->base;
            }
            else
            {
                if (precallresult != PCRLUA)
                    return;

                L->ci->flags |= LUA_CALLINFO_RETURN;
            }
        }

        if (L->status != LUA_OK)
        {
            LUAU_ASSERT(firstArg >= L->base);
            LUAU_ASSERT(isyielded(L));
            L->status = LUA_OK;

            Closure* cl = curr_func(L);

            if (cl->isC)
            {
                if (!cl->c.cont)
                {
                    luau_poscall(L, firstArg);
                }
                else
                {
                    L->base = L->ci->base;
                }
            }
            else
            {
                L->base = L->ci->base;
            }
        }
    }
    else
    {
        if (L->status == 0)
        {
            LUAU_ASSERT(L->ci == L->base_ci && firstArg >= L->base);
            if (firstArg == L->base)
                luaG_runerror(L, "cannot resume dead coroutine");

            if (luau_precall(L, firstArg - 1, LUA_MULTRET) != PCRLUA)
                return;

            L->ci->flags |= LUA_CALLINFO_RETURN;
        }
        else
        {
            LUAU_ASSERT(firstArg >= L->base);
            LUAU_ASSERT(L->status == LUA_YIELD || L->status == LUA_BREAK);
            L->status = 0;

            Closure* cl = curr_func(L);

            if (cl->isC)
            {
                if (!cl->c.cont)
                {
                    luau_poscall(L, firstArg);
                }
                else
                {
                    L->base = L->ci->base;
                }
            }
            else
            {
                L->base = L->ci->base;
            }
        }
    }

    resume_continue(L);
}

static CallInfo* resume_findhandler(lua_State* L)
{
    CallInfo* ci = L->ci;

    while (ci > L->base_ci)
    {
        if (ci->flags & LUA_CALLINFO_HANDLE)
            return ci;

        ci--;
    }

    return NULL;
}

static void restore_stack_limit(lua_State* L)
{
    LUAU_ASSERT(L->stack_last - L->stack == L->stacksize - EXTRA_STACK);
    if (L->size_ci > LUAI_MAXCALLS)
    {
        int inuse = cast_int(L->ci - L->base_ci);
        if (inuse + 1 < LUAI_MAXCALLS)
            luaD_reallocCI(L, LUAI_MAXCALLS);
    }
}

static void resume_handle(lua_State* L, void* ud)
{
    CallInfo* ci = (CallInfo*)ud;
    Closure* cl = ci_func(ci);

    LUAU_ASSERT(ci->flags & LUA_CALLINFO_HANDLE);
    LUAU_ASSERT(cl->isC && cl->c.cont);
    LUAU_ASSERT(L->status != 0);

    L->nCcalls = L->baseCcalls;

    ci->flags &= ~LUA_CALLINFO_HANDLE;

    int status = L->status;

    L->status = LUA_OK;

    if (status != LUA_ERRRUN)
        luaD_seterrorobj(L, status, L->top);

    L->base = ci->base;
    ci->top = L->top;

    ptrdiff_t old_ci = saveci(L, ci);

    int n = cl->c.cont(L, status);

    L->ci = restoreci(L, old_ci);

    luaF_close(L, L->ci->base);

    restore_stack_limit(L);

    luau_poscall(L, L->top - n);

    resume_continue(L);
}

static int resume_error(lua_State* L, const char* msg, int narg)
{
    L->top -= narg;
    setsvalue(L, L->top, luaS_new(L, msg));
    incr_top(L);
    return LUA_ERRRUN;
}

static int resume_start(lua_State* L, lua_State* from, int nargs)
{
    api_check(L, L->top - L->base >= nargs);

    if (L->status != LUA_YIELD && L->status != LUA_BREAK && (L->status != 0 || L->ci != L->base_ci))
        return resume_error(L, "cannot resume non-suspended coroutine", nargs);

    L->nCcalls = from ? from->nCcalls : 0;
    if (L->nCcalls >= LUAI_MAXCCALLS)
        return resume_error(L, "C stack overflow", nargs);

    L->baseCcalls = ++L->nCcalls;
    L->isactive = true;

    luaC_threadbarrier(L);

    return LUA_OK;
}

static int resume_finish(lua_State* L, int status)
{
    CallInfo* ch = NULL;
    while (status != LUA_OK && (ch = resume_findhandler(L)) != NULL)
    {
        if (FFlag::LuauStacklessPcall && lua_isyieldable(L) != 0 && L->global->cb.debugprotectederror)
        {
            L->global->cb.debugprotectederror(L);

            if (L->status == LUA_BREAK)
            {
                status = LUA_OK;
                break;
            }
        }

        L->status = cast_byte(status);
        status = luaD_rawrunprotected(L, resume_handle, ch);
    }

    L->nCcalls = --L->baseCcalls;

    L->baseCcalls = L->nCcalls;

    L->isactive = false;

    if (status != LUA_OK)
    {
        L->status = cast_byte(status);
        luaD_seterrorobj(L, status, L->top);
        L->ci->top = L->top;
    }
    else if (L->status == LUA_OK)
    {
        expandstacklimit(L, L->top);
    }

    return L->status;
}

int lua_resume(lua_State* L, lua_State* from, int nargs)
{
    if (int starterror = resume_start(L, from, nargs))
        return starterror;

    int status = luaD_rawrunprotected(L, resume, L->top - nargs);

    return resume_finish(L, status);
}

int lua_resumeerror(lua_State* L, lua_State* from)
{
    if (int starterror = resume_start(L, from, 1))
        return starterror;

    int status = LUA_ERRRUN;

    if (CallInfo* ci = resume_findhandler(L))
    {
        L->status = cast_byte(status);
        status = luaD_rawrunprotected(L, resume_handle, ci);
    }

    return resume_finish(L, status);
}

int lua_yield(lua_State* L, int nresults)
{
    if (L->nCcalls > L->baseCcalls)
        luaG_runerror(L, "attempt to yield across metamethod/C-call boundary");
    L->base = L->top - nresults;
    L->status = LUA_YIELD;
    return -1;
}

int lua_break(lua_State* L)
{
    if (L->nCcalls > L->baseCcalls)
        luaG_runerror(L, "attempt to break across metamethod/C-call boundary");
    L->status = LUA_BREAK;
    return -1;
}

int lua_isyieldable(lua_State* L)
{
    return (L->nCcalls <= L->baseCcalls);
}

static void callerrfunc(lua_State* L, void* ud)
{
    StkId errfunc = cast_to(StkId, ud);

    setobj2s(L, L->top, L->top - 1);
    setobj2s(L, L->top - 1, errfunc);
    incr_top(L);

    luaD_callny(L, L->top - 2, 1);
}

int luaD_pcall(lua_State* L, Pfunc func, void* u, ptrdiff_t old_top, ptrdiff_t ef)
{
    unsigned short oldnCcalls = L->nCcalls;
    unsigned short oldbaseCcalls = L->baseCcalls;
    ptrdiff_t old_ci = saveci(L, L->ci);
    bool oldactive = L->isactive;
    int status = luaD_rawrunprotected(L, func, u);
    if (status != 0)
    {
        int errstatus = status;

        if (ef)
        {
            if (status != LUA_ERRRUN)
                luaD_seterrorobj(L, status, L->top);

            int err = luaD_rawrunprotected(L, callerrfunc, restorestack(L, ef));

            if (err == 0)
                errstatus = LUA_ERRRUN;
            else if (status == LUA_ERRMEM && err == LUA_ERRMEM)
                errstatus = LUA_ERRMEM;
            else
                errstatus = status = LUA_ERRERR;
        }

        if (!oldactive)
            L->isactive = false;

        bool yieldable = L->nCcalls <= L->baseCcalls;

        L->nCcalls = oldnCcalls;
        L->baseCcalls = oldbaseCcalls;

        if (yieldable && L->global->cb.debugprotectederror)
        {
            L->global->cb.debugprotectederror(L);

            if (L->status == LUA_BREAK)
                return 0;
        }

        StkId oldtop = restorestack(L, old_top);
        luaF_close(L, oldtop);
        luaD_seterrorobj(L, errstatus, oldtop);
        L->ci = restoreci(L, old_ci);
        L->base = L->ci->base;
        restore_stack_limit(L);
    }
    return status;
}
