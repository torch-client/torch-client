#ifndef MLUA_SHIM_ERRNO_H
#define MLUA_SHIM_ERRNO_H

#ifdef __cplusplus
extern "C" {
#endif

int* mlua_shim_errno(void);
#define errno (*mlua_shim_errno())

#define EDOM 33
#define EINVAL 22
#define ERANGE 34

#ifdef __cplusplus
}
#endif

#endif
