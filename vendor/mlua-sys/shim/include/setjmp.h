#ifndef MLUA_SHIM_SETJMP_H
#define MLUA_SHIM_SETJMP_H

#ifdef __cplusplus
extern "C" {
#endif

typedef struct
{
    unsigned long words[4];
} jmp_buf[1];

int setjmp(jmp_buf env) __attribute__((returns_twice));
__attribute__((noreturn)) void longjmp(jmp_buf env, int value);

#ifdef __cplusplus
}
#endif

#endif
