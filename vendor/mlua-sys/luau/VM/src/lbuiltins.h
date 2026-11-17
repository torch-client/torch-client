#pragma once

#include "lobject.h"

typedef int (*luau_FastFunction)(lua_State* L, StkId res, TValue* arg0, int nresults, StkId args, int nparams);

extern const luau_FastFunction luauF_table[256];
