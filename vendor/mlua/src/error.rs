use std::error::Error as StdError;
use std::fmt;
use std::io::Error as IoError;
use std::net::AddrParseError;
use std::result::Result as StdResult;
use std::str::Utf8Error;
use std::string::String as StdString;
use std::sync::Arc;

use crate::private::Sealed;

#[cfg(feature = "error-send")]
type DynStdError = dyn StdError + Send + Sync;

#[cfg(not(feature = "error-send"))]
type DynStdError = dyn StdError;

#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum Error {
    SyntaxError {
        message: StdString,
        incomplete_input: bool,
    },
    RuntimeError(StdString),
    MemoryError(StdString),
    #[cfg(any(feature = "lua53", feature = "lua52", doc))]
    #[cfg_attr(docsrs, doc(cfg(any(feature = "lua53", feature = "lua52"))))]
    GarbageCollectorError(StdString),
    SafetyError(StdString),
    MemoryControlNotAvailable,
    RecursiveMutCallback,
    CallbackDestructed,
    StackError,
    BindError,
    BadArgument {
        to: Option<StdString>,
        pos: usize,
        name: Option<StdString>,
        cause: Arc<Error>,
    },
    ToLuaConversionError {
        from: String,
        to: &'static str,
        message: Option<StdString>,
    },
    FromLuaConversionError {
        from: &'static str,
        to: String,
        message: Option<StdString>,
    },
    CoroutineUnresumable,
    UserDataTypeMismatch,
    UserDataDestructed,
    UserDataBorrowError,
    UserDataBorrowMutError,
    MetaMethodRestricted(StdString),
    MetaMethodTypeError {
        method: StdString,
        type_name: &'static str,
        message: Option<StdString>,
    },
    MismatchedRegistryKey,
    CallbackError {
        traceback: StdString,
        cause: Arc<Error>,
    },
    PreviouslyResumedPanic,
    #[cfg(feature = "serde")]
    #[cfg_attr(docsrs, doc(cfg(feature = "serde")))]
    SerializeError(StdString),
    #[cfg(feature = "serde")]
    #[cfg_attr(docsrs, doc(cfg(feature = "serde")))]
    DeserializeError(StdString),
    ExternalError(Arc<DynStdError>),
    WithContext {
        context: StdString,
        cause: Arc<Error>,
    },
}

pub type Result<T> = StdResult<T, Error>;

#[cfg(not(tarpaulin_include))]
impl fmt::Display for Error {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::SyntaxError { message, .. } => write!(fmt, "syntax error: {message}"),
            Error::RuntimeError(msg) => write!(fmt, "runtime error: {msg}"),
            Error::MemoryError(msg) => {
                write!(fmt, "memory error: {msg}")
            }
            #[cfg(any(feature = "lua53", feature = "lua52"))]
            Error::GarbageCollectorError(msg) => {
                write!(fmt, "garbage collector error: {msg}")
            }
            Error::SafetyError(msg) => {
                write!(fmt, "safety error: {msg}")
            },
            Error::MemoryControlNotAvailable => {
                write!(fmt, "memory control is not available")
            }
            Error::RecursiveMutCallback => write!(fmt, "mutable callback called recursively"),
            Error::CallbackDestructed => write!(
                fmt,
                "a destructed callback or destructed userdata method was called"
            ),
            Error::StackError => write!(
                fmt,
                "out of Lua stack, too many arguments to a Lua function or too many return values from a callback"
            ),
            Error::BindError => write!(
                fmt,
                "too many arguments to Function::bind"
            ),
            Error::BadArgument { to, pos, name, cause } => {
                if let Some(name) = name {
                    write!(fmt, "bad argument `{name}`")?;
                } else {
                    write!(fmt, "bad argument #{pos}")?;
                }
                if let Some(to) = to {
                    write!(fmt, " to `{to}`")?;
                }
                write!(fmt, ": {cause}")
            },
            Error::ToLuaConversionError { from, to, message } => {
                write!(fmt, "error converting {from} to Lua {to}")?;
                match message {
                    None => Ok(()),
                    Some(message) => write!(fmt, " ({message})"),
                }
            }
            Error::FromLuaConversionError { from, to, message } => {
                write!(fmt, "error converting Lua {from} to {to}")?;
                match message {
                    None => Ok(()),
                    Some(message) => write!(fmt, " ({message})"),
                }
            }
            Error::CoroutineUnresumable => write!(fmt, "coroutine is non-resumable"),
            Error::UserDataTypeMismatch => write!(fmt, "userdata is not expected type"),
            Error::UserDataDestructed => write!(fmt, "userdata has been destructed"),
            Error::UserDataBorrowError => write!(fmt, "error borrowing userdata"),
            Error::UserDataBorrowMutError => write!(fmt, "error mutably borrowing userdata"),
            Error::MetaMethodRestricted(method) => write!(fmt, "metamethod {method} is restricted"),
            Error::MetaMethodTypeError { method, type_name, message } => {
                write!(fmt, "metamethod {method} has unsupported type {type_name}")?;
                match message {
                    None => Ok(()),
                    Some(message) => write!(fmt, " ({message})"),
                }
            }
            Error::MismatchedRegistryKey => {
                write!(fmt, "RegistryKey used from different Lua state")
            }
            Error::CallbackError { cause, traceback } => {
                let (mut cause, mut full_traceback) = (cause, None);
                while let Error::CallbackError { cause: cause2, traceback: traceback2 } = &**cause {
                    cause = cause2;
                    full_traceback = Some(traceback2);
                }
                writeln!(fmt, "{cause}")?;
                if let Some(full_traceback) = full_traceback {
                    let traceback = traceback.trim_start_matches("stack traceback:");
                    let traceback = traceback.trim_start().trim_end();
                    if let Some(pos) = full_traceback.find(traceback) {
                        write!(fmt, "{}", &full_traceback[..pos])?;
                        writeln!(fmt, ">{}", &full_traceback[pos..].trim_end())?;
                    } else {
                        writeln!(fmt, "{}", full_traceback.trim_end())?;
                    }
                } else {
                    writeln!(fmt, "{}", traceback.trim_end())?;
                }
                Ok(())
            }
            Error::PreviouslyResumedPanic => {
                write!(fmt, "previously resumed panic returned again")
            }
            #[cfg(feature = "serde")]
            Error::SerializeError(err) => {
                write!(fmt, "serialize error: {err}")
            },
            #[cfg(feature = "serde")]
            Error::DeserializeError(err) => {
                write!(fmt, "deserialize error: {err}")
            },
            Error::ExternalError(err) => err.fmt(fmt),
            Error::WithContext { context, cause } => {
                writeln!(fmt, "{context}")?;
                write!(fmt, "{cause}")
            }
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Error::CallbackError { .. } => None,
            Error::ExternalError(err) => err.source(),
            Error::WithContext { cause, .. } => Self::source(cause),
            _ => None,
        }
    }
}

impl Error {
    #[inline]
    pub fn runtime<S: fmt::Display>(message: S) -> Self {
        Error::RuntimeError(message.to_string())
    }

    #[inline]
    pub fn external<T: Into<Box<DynStdError>>>(err: T) -> Self {
        Error::ExternalError(err.into().into())
    }

    pub fn downcast_ref<T>(&self) -> Option<&T>
    where
        T: StdError + 'static,
    {
        match self {
            Error::ExternalError(err) => err.downcast_ref(),
            Error::WithContext { cause, .. } => Self::downcast_ref(cause),
            _ => None,
        }
    }

    pub fn chain(&self) -> impl Iterator<Item = &(dyn StdError + 'static)> {
        Chain {
            root: self,
            current: None,
        }
    }

    #[doc(hidden)]
    pub fn parent(&self) -> Option<&Error> {
        match self {
            Error::CallbackError { cause, .. } => Some(cause.as_ref()),
            Error::WithContext { cause, .. } => Some(cause.as_ref()),
            _ => None,
        }
    }

    pub(crate) fn bad_self_argument(to: &str, cause: Error) -> Self {
        Error::BadArgument {
            to: Some(to.to_string()),
            pos: 1,
            name: Some("self".to_string()),
            cause: Arc::new(cause),
        }
    }

    pub(crate) fn from_lua_conversion(
        from: &'static str,
        to: impl ToString,
        message: impl Into<Option<String>>,
    ) -> Self {
        Error::FromLuaConversionError {
            from,
            to: to.to_string(),
            message: message.into(),
        }
    }
}

pub trait ExternalError {
    fn into_lua_err(self) -> Error;
}

impl<E: Into<Box<DynStdError>>> ExternalError for E {
    fn into_lua_err(self) -> Error {
        Error::external(self)
    }
}

pub trait ExternalResult<T> {
    fn into_lua_err(self) -> Result<T>;
}

impl<T, E> ExternalResult<T> for StdResult<T, E>
where
    E: ExternalError,
{
    fn into_lua_err(self) -> Result<T> {
        self.map_err(|e| e.into_lua_err())
    }
}

pub trait ErrorContext: Sealed {
    fn context<C: fmt::Display>(self, context: C) -> Self;

    fn with_context<C: fmt::Display>(self, f: impl FnOnce(&Error) -> C) -> Self;
}

impl ErrorContext for Error {
    fn context<C: fmt::Display>(self, context: C) -> Self {
        let context = context.to_string();
        match self {
            Error::WithContext { cause, .. } => Error::WithContext { context, cause },
            _ => Error::WithContext {
                context,
                cause: Arc::new(self),
            },
        }
    }

    fn with_context<C: fmt::Display>(self, f: impl FnOnce(&Error) -> C) -> Self {
        let context = f(&self).to_string();
        match self {
            Error::WithContext { cause, .. } => Error::WithContext { context, cause },
            _ => Error::WithContext {
                context,
                cause: Arc::new(self),
            },
        }
    }
}

impl<T> ErrorContext for Result<T> {
    fn context<C: fmt::Display>(self, context: C) -> Self {
        self.map_err(|err| err.context(context))
    }

    fn with_context<C: fmt::Display>(self, f: impl FnOnce(&Error) -> C) -> Self {
        self.map_err(|err| err.with_context(f))
    }
}

impl From<AddrParseError> for Error {
    fn from(err: AddrParseError) -> Self {
        Error::external(err)
    }
}

impl From<IoError> for Error {
    fn from(err: IoError) -> Self {
        Error::external(err)
    }
}

impl From<Utf8Error> for Error {
    fn from(err: Utf8Error) -> Self {
        Error::external(err)
    }
}

#[cfg(feature = "serde")]
impl serde::ser::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Self::SerializeError(msg.to_string())
    }
}

#[cfg(feature = "serde")]
impl serde::de::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Self::DeserializeError(msg.to_string())
    }
}

#[cfg(feature = "anyhow")]
impl From<anyhow::Error> for Error {
    fn from(err: anyhow::Error) -> Self {
        match err.downcast::<Self>() {
            Ok(err) => err,
            Err(err) => Error::external(err),
        }
    }
}

struct Chain<'a> {
    root: &'a Error,
    current: Option<&'a (dyn StdError + 'static)>,
}

impl<'a> Iterator for Chain<'a> {
    type Item = &'a (dyn StdError + 'static);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let error: Option<&dyn StdError> = match self.current {
                None => {
                    self.current = Some(self.root);
                    self.current
                }
                Some(current) => match current.downcast_ref::<Error>()? {
                    Error::BadArgument { cause, .. }
                    | Error::CallbackError { cause, .. }
                    | Error::WithContext { cause, .. } => {
                        self.current = Some(&**cause);
                        self.current
                    }
                    Error::ExternalError(err) => {
                        self.current = Some(&**err);
                        self.current
                    }
                    _ => None,
                },
            };

            if let Some(Error::ExternalError(_)) = error?.downcast_ref::<Error>() {
                continue;
            }

            return self.current;
        }
    }
}

#[cfg(test)]
mod assertions {
    use super::*;

    #[cfg(not(feature = "error-send"))]
    static_assertions::assert_not_impl_any!(Error: Send, Sync);
    #[cfg(feature = "send")]
    static_assertions::assert_impl_all!(Error: Send, Sync);
}
