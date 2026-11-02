use std::cell::RefCell;
use std::marker::PhantomData;
use std::mem;

use crate::error::{Error, Result};
use crate::function::Function;
use crate::state::{Lua, LuaGuard, RawLua};
use crate::traits::{FromLuaMulti, IntoLuaMulti};
use crate::types::{Callback, CallbackUpvalue, ScopedCallback, ValueRef};
use crate::userdata::{AnyUserData, UserData, UserDataRegistry, UserDataStorage};
use crate::util::{self, check_stack, get_metatable_ptr, get_userdata, take_userdata, StackGuard};

pub struct Scope<'scope, 'env: 'scope> {
    lua: LuaGuard,
    destructors: Destructors<'env>,
    user_destructors: UserDestructors<'env>,
    _scope_invariant: PhantomData<&'scope mut &'scope ()>,
    _env_invariant: PhantomData<&'env mut &'env ()>,
}

type DestructorCallback<'a> = Box<dyn FnOnce(&RawLua, ValueRef) -> Vec<Box<dyn FnOnce() + 'a>>>;

struct Destructors<'a>(RefCell<Vec<(ValueRef, DestructorCallback<'a>)>>);

struct UserDestructors<'a>(RefCell<Vec<Box<dyn FnOnce() + 'a>>>);

impl<'scope, 'env: 'scope> Scope<'scope, 'env> {
    pub(crate) fn new(lua: LuaGuard) -> Self {
        Scope {
            lua,
            destructors: Destructors(RefCell::new(Vec::new())),
            user_destructors: UserDestructors(RefCell::new(Vec::new())),
            _scope_invariant: PhantomData,
            _env_invariant: PhantomData,
        }
    }

    pub fn create_function<F, A, R>(&'scope self, func: F) -> Result<Function>
    where
        F: Fn(&Lua, A) -> Result<R> + 'scope,
        A: FromLuaMulti,
        R: IntoLuaMulti,
    {
        unsafe {
            self.create_callback(Box::new(move |rawlua, nargs| {
                let args = A::from_stack_args(nargs, 1, None, rawlua)?;
                func(rawlua.lua(), args)?.push_into_stack_multi(rawlua)
            }))
        }
    }

    pub fn create_function_mut<F, A, R>(&'scope self, func: F) -> Result<Function>
    where
        F: FnMut(&Lua, A) -> Result<R> + 'scope,
        A: FromLuaMulti,
        R: IntoLuaMulti,
    {
        let func = RefCell::new(func);
        self.create_function(move |lua, args| {
            (*func.try_borrow_mut().map_err(|_| Error::RecursiveMutCallback)?)(lua, args)
        })
    }

    pub fn create_userdata_ref<T>(&'scope self, data: &'env T) -> Result<AnyUserData>
    where
        T: UserData + 'static,
    {
        let ud = unsafe { self.lua.make_userdata(UserDataStorage::new_ref(data)) }?;
        self.seal_userdata::<T>(&ud);
        Ok(ud)
    }

    pub fn create_userdata_ref_mut<T>(&'scope self, data: &'env mut T) -> Result<AnyUserData>
    where
        T: UserData + 'static,
    {
        let ud = unsafe { self.lua.make_userdata(UserDataStorage::new_ref_mut(data)) }?;
        self.seal_userdata::<T>(&ud);
        Ok(ud)
    }

    pub fn create_any_userdata_ref<T>(&'scope self, data: &'env T) -> Result<AnyUserData>
    where
        T: 'static,
    {
        let ud = unsafe { self.lua.make_any_userdata(UserDataStorage::new_ref(data)) }?;
        self.seal_userdata::<T>(&ud);
        Ok(ud)
    }

    pub fn create_any_userdata_ref_mut<T>(&'scope self, data: &'env mut T) -> Result<AnyUserData>
    where
        T: 'static,
    {
        let ud = unsafe { self.lua.make_any_userdata(UserDataStorage::new_ref_mut(data)) }?;
        self.seal_userdata::<T>(&ud);
        Ok(ud)
    }

    pub fn create_userdata<T>(&'scope self, data: T) -> Result<AnyUserData>
    where
        T: UserData + 'env,
    {
        let state = self.lua.state();
        unsafe {
            let _sg = StackGuard::new(state);
            check_stack(state, 3)?;

            let protect = !self.lua.unlikely_memory_error();
            #[cfg(feature = "luau")]
            let ud_ptr = {
                let data = UserDataStorage::new_scoped(data);
                util::push_userdata(state, data, protect)?
            };
            #[cfg(not(feature = "luau"))]
            let ud_ptr = util::push_uninit_userdata::<UserDataStorage<T>>(state, protect)?;

            let mut registry = UserDataRegistry::new_unique(self.lua.lua(), ud_ptr as *mut _);
            T::register(&mut registry);
            self.lua.push_userdata_metatable(registry.into_raw())?;
            let mt_ptr = ffi::lua_topointer(state, -1);
            self.lua.register_userdata_metatable(mt_ptr, None);

            #[cfg(not(feature = "luau"))]
            std::ptr::write(ud_ptr, UserDataStorage::new_scoped(data));
            ffi::lua_setmetatable(state, -2);

            let ud = AnyUserData(self.lua.pop_ref());
            self.seal_userdata::<T>(&ud);

            Ok(ud)
        }
    }

    pub fn create_any_userdata<T>(
        &'scope self,
        data: T,
        register: impl FnOnce(&mut UserDataRegistry<T>),
    ) -> Result<AnyUserData>
    where
        T: 'env,
    {
        let state = self.lua.state();
        let ud = unsafe {
            let _sg = StackGuard::new(state);
            check_stack(state, 3)?;

            let protect = !self.lua.unlikely_memory_error();
            #[cfg(feature = "luau")]
            let ud_ptr = {
                let data = UserDataStorage::new_scoped(data);
                util::push_userdata(state, data, protect)?
            };
            #[cfg(not(feature = "luau"))]
            let ud_ptr = util::push_uninit_userdata::<UserDataStorage<T>>(state, protect)?;

            let mut registry = UserDataRegistry::new_unique(self.lua.lua(), ud_ptr as *mut _);
            register(&mut registry);
            self.lua.push_userdata_metatable(registry.into_raw())?;
            let mt_ptr = ffi::lua_topointer(state, -1);
            self.lua.register_userdata_metatable(mt_ptr, None);

            #[cfg(not(feature = "luau"))]
            std::ptr::write(ud_ptr, UserDataStorage::new_scoped(data));
            ffi::lua_setmetatable(state, -2);

            AnyUserData(self.lua.pop_ref())
        };
        self.seal_userdata::<T>(&ud);
        Ok(ud)
    }

    pub fn add_destructor(&'scope self, destructor: impl FnOnce() + 'env) {
        self.user_destructors.0.borrow_mut().push(Box::new(destructor));
    }

    unsafe fn create_callback(&'scope self, f: ScopedCallback<'scope>) -> Result<Function> {
        let f = mem::transmute::<ScopedCallback, Callback>(f);
        let f = self.lua.create_callback(f)?;

        let destructor: DestructorCallback = Box::new(|rawlua, vref| {
            let ref_thread = rawlua.ref_thread();
            ffi::lua_getupvalue(ref_thread, vref.index, 1);
            let upvalue = get_userdata::<CallbackUpvalue>(ref_thread, -1);
            let data = (*upvalue).data.take();
            ffi::lua_pop(ref_thread, 1);
            vec![Box::new(move || drop(data))]
        });
        self.destructors.0.borrow_mut().push((f.0.clone(), destructor));

        Ok(f)
    }

    fn seal_userdata<T: 'env>(&self, ud: &AnyUserData) {
        let destructor: DestructorCallback = Box::new(|rawlua, vref| unsafe {
            match rawlua.get_userdata_ref_type_id(&vref) {
                Ok(Some(_)) => {}
                Ok(None) => {
                    let mt_ptr = get_metatable_ptr(rawlua.ref_thread(), vref.index);
                    rawlua.deregister_userdata_metatable(mt_ptr);
                }
                Err(_) => return vec![],
            }

            let data = take_userdata::<UserDataStorage<T>>(rawlua.ref_thread(), vref.index);
            vec![Box::new(move || drop(data))]
        });
        self.destructors.0.borrow_mut().push((ud.0.clone(), destructor));
    }
}

impl Drop for Destructors<'_> {
    fn drop(&mut self) {

        let destructors = mem::take(&mut *self.0.borrow_mut());
        if let Some(lua) = destructors.first().map(|(vref, _)| vref.lua.lock()) {
            let to_drop = destructors
                .into_iter()
                .flat_map(|(vref, destructor)| destructor(&lua, vref))
                .collect::<Vec<_>>();

            drop(to_drop);
        }
    }
}

impl Drop for UserDestructors<'_> {
    fn drop(&mut self) {
        let destructors = mem::take(&mut *self.0.borrow_mut());
        for destructor in destructors {
            destructor();
        }
    }
}
