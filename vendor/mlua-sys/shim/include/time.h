#ifndef MLUA_SHIM_TIME_H
#define MLUA_SHIM_TIME_H

#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef long long time_t;
typedef long long clock_t;

#define CLOCKS_PER_SEC 1000000LL

struct tm
{
    int tm_sec;
    int tm_min;
    int tm_hour;
    int tm_mday;
    int tm_mon;
    int tm_year;
    int tm_wday;
    int tm_yday;
    int tm_isdst;
};

struct timespec
{
    time_t tv_sec;
    long tv_nsec;
};

time_t time(time_t* out);
clock_t clock(void);
double difftime(time_t end, time_t start);
time_t mktime(struct tm* tm);
struct tm* gmtime_r(const time_t* t, struct tm* out);
struct tm* localtime_r(const time_t* t, struct tm* out);
struct tm* localtime(const time_t* t);
size_t strftime(char* buf, size_t cap, const char* fmt, const struct tm* tm);

#ifdef __cplusplus
}
#endif

#endif
