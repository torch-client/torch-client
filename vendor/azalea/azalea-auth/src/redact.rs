use std::fmt;

pub struct Redacted;

impl fmt::Debug for Redacted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}

#[doc(hidden)]
#[macro_export]
macro_rules! redacted_debug {
    ($ty:ident { $($shown:ident),* ; $($hidden:ident),* }) => {
        impl ::std::fmt::Debug for $ty {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                let Self { $($shown,)* $($hidden: _,)* } = self;
                f.debug_struct(stringify!($ty))
                    $(.field(stringify!($shown), $shown))*
                    $(.field(stringify!($hidden), &$crate::redact::Redacted))*
                    .finish()
            }
        }
    };
}

#[cfg(test)]
mod tests {
    struct Login {
        name: String,
        token: String,
    }
    redacted_debug!(Login { name ; token });

    #[test]
    fn a_hidden_field_is_not_printed() {
        let login = Login { name: "Steve".into(), token: "eyJhbGciOi".into() };
        let printed = format!("{login:?} {login:#?}");
        assert!(printed.contains("Steve"));
        assert!(printed.contains("<redacted>"));
        assert!(!printed.contains("eyJhbGciOi"));
    }
}
