macro_rules! named_cycle {
    ($ty:ty) => {
        impl $ty {
            #[allow(dead_code)]
            pub fn from_serialized_name(name: &str) -> Option<$ty> {
                <$ty>::ALL.into_iter().find(|v| v.serialized_name() == name)
            }

            #[allow(dead_code)]
            pub fn next(self) -> $ty {
                let i = <$ty>::ALL.iter().position(|v| *v == self).unwrap_or(0);
                <$ty>::ALL[(i + 1) % <$ty>::ALL.len()]
            }
        }
    };
}

pub(crate) use named_cycle;
