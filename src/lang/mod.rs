include!(concat!(env!("OUT_DIR"), "/lang_tables.rs"));

static EN_TABLE: [&str; KEYS.len()] = EN;
#[cfg(feature = "multi_language")]
static RU_TABLE: [&str; KEYS.len()] = RU;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Key(u16);

#[allow(
    dead_code,
    reason = "the module table and Lua rows are the callers of the runtime half"
)]
impl Key {
    pub(crate) const NONE: Key = Key(u16::MAX);

    pub(crate) const fn of(key: &str) -> Key {
        match search(key.as_bytes()) {
            Some(i) => Key(i as u16),
            None => panic!("{}", key),
        }
    }

    pub(crate) fn lookup(key: &str) -> Option<Key> {
        search(key.as_bytes()).map(|i| Key(i as u16))
    }

    pub(crate) const fn index(self) -> usize {
        self.0 as usize
    }

    pub(crate) fn get(self) -> &'static str {
        if self == Self::NONE {
            return "";
        }
        match active() {
            Lang::EnUs => EN_TABLE[self.index()],
            #[cfg(feature = "multi_language")]
            Lang::RuRu => RU_TABLE[self.index()],
            #[cfg(not(feature = "multi_language"))]
            Lang::RuRu => EN_TABLE[self.index()],
        }
    }

    pub(crate) fn or(self, english: &'static str) -> &'static str {
        if self == Self::NONE {
            english
        } else {
            self.get()
        }
    }

    pub(crate) fn english(self) -> &'static str {
        if self == Self::NONE {
            ""
        } else {
            EN_TABLE[self.index()]
        }
    }
}

const fn search(key: &[u8]) -> Option<usize> {
    let (mut lo, mut hi) = (0, KEYS.len());
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        match compare(KEYS[mid].as_bytes(), key) {
            std::cmp::Ordering::Less => lo = mid + 1,
            std::cmp::Ordering::Greater => hi = mid,
            std::cmp::Ordering::Equal => return Some(mid),
        }
    }
    None
}

const fn compare(a: &[u8], b: &[u8]) -> std::cmp::Ordering {
    let mut i = 0;
    while i < a.len() && i < b.len() {
        if a[i] != b[i] {
            return if a[i] < b[i] {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            };
        }
        i += 1;
    }
    if a.len() < b.len() {
        std::cmp::Ordering::Less
    } else if a.len() > b.len() {
        std::cmp::Ordering::Greater
    } else {
        std::cmp::Ordering::Equal
    }
}

#[cfg(not(feature = "multi_language"))]
#[cfg_attr(not(lua_mods), allow(unused_macros))]
macro_rules! tr {
    ($key:literal) => {
        const { $crate::lang::EN[$crate::lang::Key::of($key).index()] }
    };
}
#[cfg(feature = "multi_language")]
#[cfg_attr(not(lua_mods), allow(unused_macros))]
macro_rules! tr {
    ($key:literal) => {
        const { $crate::lang::Key::of($key) }.get()
    };
}
#[cfg_attr(not(lua_mods), allow(unused_imports))]
pub(crate) use tr;

#[allow(
    unused_macros,
    reason = "the first formatted client string comes with the module table"
)]
macro_rules! trf {
    ($key:literal $(, $arg:expr)* $(,)?) => {
        $crate::util::placeholders::substitute(
            $crate::lang::tr!($key),
            &[$(::std::string::ToString::to_string(&$arg)),*],
        )
    };
}
#[allow(
    unused_imports,
    reason = "the first formatted client string comes with the module table"
)]
pub(crate) use trf;

#[allow(
    unused_macros,
    reason = "the callers are behind lua_mods and seed_cracker"
)]
macro_rules! trn {
    ($key:literal, $n:expr $(, $arg:expr)* $(,)?) => {{
        let n = $n;
        const FORMS: [$crate::lang::Key; 3] = [
            $crate::lang::Key::of(concat!($key, ".one")),
            $crate::lang::Key::of(concat!($key, ".few")),
            $crate::lang::Key::of(concat!($key, ".many")),
        ];
        $crate::util::placeholders::substitute(
            $crate::lang::plural(FORMS, n),
            &[::std::string::ToString::to_string(&n) $(, ::std::string::ToString::to_string(&$arg))*],
        )
    }};
}
#[allow(
    unused_imports,
    reason = "the callers are behind lua_mods and seed_cracker"
)]
pub(crate) use trn;

#[allow(
    dead_code,
    reason = "the callers of trn! are behind lua_mods and seed_cracker"
)]
pub(crate) fn plural(forms: [Key; 3], n: impl Count) -> &'static str {
    forms[active().plural(n.count()) as usize].get()
}

pub(crate) trait Count: Copy {
    fn count(self) -> u64;
}

macro_rules! count_unsigned {
    ($($t:ty),*) => {$(
        impl Count for $t {
            fn count(self) -> u64 {
                self as u64
            }
        }
    )*};
}
count_unsigned!(u8, u16, u32, u64, usize);

macro_rules! count_signed {
    ($($t:ty),*) => {$(
        impl Count for $t {
            fn count(self) -> u64 {
                self.unsigned_abs() as u64
            }
        }
    )*};
}
count_signed!(i8, i16, i32, i64, isize);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub(crate) enum Plural {
    One = 0,
    Few = 1,
    Many = 2,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub(crate) enum Lang {
    EnUs = 0,
    #[cfg_attr(not(feature = "multi_language"), allow(dead_code))]
    RuRu = 1,
}

impl Lang {
    #[cfg_attr(not(feature = "multi_language"), allow(dead_code))]
    pub(crate) fn code(self) -> &'static str {
        match self {
            Lang::EnUs => "en_us",
            Lang::RuRu => "ru_ru",
        }
    }

    #[cfg_attr(not(feature = "multi_language"), allow(dead_code))]
    fn from_code(code: &str) -> Lang {
        match primary(code).as_deref() {
            Some("ru") => Lang::RuRu,
            _ => Lang::EnUs,
        }
    }

    pub(crate) fn plural(self, n: u64) -> Plural {
        match self {
            Lang::EnUs => {
                if n == 1 {
                    Plural::One
                } else {
                    Plural::Many
                }
            }
            Lang::RuRu => {
                let (a, b) = (n % 10, n % 100);
                if a == 1 && b != 11 {
                    Plural::One
                } else if (2..=4).contains(&a) && !(12..=14).contains(&b) {
                    Plural::Few
                } else {
                    Plural::Many
                }
            }
        }
    }
}

#[cfg_attr(not(feature = "multi_language"), allow(dead_code))]
fn primary(code: &str) -> Option<String> {
    let first = code.split(['_', '-']).next()?.trim();
    (!first.is_empty()).then(|| first.to_ascii_lowercase())
}

#[cfg_attr(not(feature = "multi_language"), allow(dead_code))]
pub(crate) fn supported(code: &str) -> bool {
    match primary(code).as_deref() {
        Some("en") => true,
        Some("ru") => cfg!(feature = "multi_language"),
        _ => false,
    }
}

#[inline]
pub(crate) fn active() -> Lang {
    #[cfg(feature = "multi_language")]
    {
        state::active()
    }
    #[cfg(not(feature = "multi_language"))]
    {
        Lang::EnUs
    }
}

#[cfg_attr(not(feature = "asset_download"), allow(dead_code))]
pub(crate) fn set_code(code: &str) {
    #[cfg(feature = "multi_language")]
    state::set_code(code);
    #[cfg(not(feature = "multi_language"))]
    let _ = code;
}

#[allow(
    dead_code,
    reason = "read by callers that cache wrapped text, from the module table on"
)]
pub(crate) fn epoch() -> u32 {
    #[cfg(feature = "multi_language")]
    {
        state::epoch()
    }
    #[cfg(not(feature = "multi_language"))]
    {
        0
    }
}

#[cfg(all(feature = "multi_language", feature = "asset_download"))]
pub(crate) fn detected() -> bool {
    state::detected()
}

#[cfg(feature = "multi_language")]
mod state {
    use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering};

    use super::Lang;

    const UNSET: u8 = u8::MAX;

    static ACTIVE: AtomicU8 = AtomicU8::new(UNSET);
    static EPOCH: AtomicU32 = AtomicU32::new(0);
    static FROM_LOCALE: AtomicBool = AtomicBool::new(false);

    fn decode(value: u8) -> Lang {
        if value == Lang::RuRu as u8 {
            Lang::RuRu
        } else {
            Lang::EnUs
        }
    }

    pub(crate) fn active() -> Lang {
        match ACTIVE.load(Ordering::Acquire) {
            UNSET => resolve(),
            value => decode(value),
        }
    }

    pub(crate) fn epoch() -> u32 {
        active();
        EPOCH.load(Ordering::Acquire)
    }

    pub(crate) fn set_code(code: &str) {
        let lang = Lang::from_code(code);
        if ACTIVE.swap(lang as u8, Ordering::AcqRel) != lang as u8 {
            EPOCH.fetch_add(1, Ordering::AcqRel);
        }
        FROM_LOCALE.store(false, Ordering::Release);
    }

    #[cfg(feature = "asset_download")]
    pub(crate) fn detected() -> bool {
        active();
        FROM_LOCALE.load(Ordering::Acquire)
    }

    #[cold]
    fn resolve() -> Lang {
        let (lang, detected) = initial();
        if detected {
            FROM_LOCALE.store(true, Ordering::Release);
        }
        match ACTIVE.compare_exchange(UNSET, lang as u8, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => lang,
            Err(value) => {
                if detected && value != lang as u8 {
                    FROM_LOCALE.store(false, Ordering::Release);
                }
                decode(value)
            }
        }
    }

    fn initial() -> (Lang, bool) {
        if cfg!(test) {
            return (Lang::EnUs, false);
        }
        #[cfg(feature = "asset_download")]
        if crate::platform::storage::LANGUAGE.load().is_some() {
            return (Lang::from_code(&crate::client::language::current()), false);
        }
        let wanted = crate::platform::locale::system();
        match wanted.iter().find(|code| super::supported(code)) {
            Some(code) => {
                let lang = Lang::from_code(code);
                crate::log_info!(
                    "locale",
                    "system language {code}, client strings in {}",
                    lang.code()
                );
                (lang, lang != Lang::EnUs)
            }
            None => {
                if !wanted.is_empty() {
                    crate::log_info!(
                        "locale",
                        "no client table for the system languages {wanted:?}, using English"
                    );
                }
                (Lang::EnUs, false)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_key_is_found() {
        assert!(KEYS.windows(2).all(|w| w[0].as_bytes() < w[1].as_bytes()));
        for (i, key) in KEYS.iter().enumerate() {
            assert_eq!(Key::of(key).index(), i);
            assert_eq!(Key::lookup(key), Some(Key::of(key)));
        }
        assert_eq!(Key::lookup("clickgui.no_such_key"), None);
        assert_eq!(Key::lookup(""), None);
        const DISMISS: Key = Key::of("clickgui.dismiss");
        assert_eq!(DISMISS.english(), "Dismiss");
    }

    #[test]
    fn none_has_no_text() {
        assert_eq!(Key::NONE.get(), "");
        assert_eq!(Key::NONE.english(), "");
        assert_eq!(Key::NONE.or("Lua row"), "Lua row");
        let dismiss = Key::of("clickgui.dismiss");
        assert_eq!(dismiss.or("Lua row"), dismiss.get());
    }

    #[test]
    fn substitution_follows_java_formatter() {
        let sub = crate::util::placeholders::substitute;
        let args = ["a".to_owned(), "b".to_owned()];
        assert_eq!(sub("%s and %s", &args), "a and b");
        assert_eq!(sub("%2$s then %1$s", &args), "b then a");
        assert_eq!(sub("%2$s %s", &args), "b a");
        assert_eq!(sub("100%%", &args), "100%");
        assert_eq!(sub("%s %s %s", &args), "a b %s");
        assert_eq!(sub("%3$s", &args), "%3$s");
        assert_eq!(sub("%d and %", &args), "%d and %");
        assert_eq!(sub("Закрыть %s", &args), "Закрыть a");
        assert_eq!(trf!("clickgui.dismiss", 1, "x"), tr!("clickgui.dismiss"));
    }

    #[test]
    fn plural_forms_follow_cldr() {
        use Plural::{Few, Many, One};
        let counts = [0, 1, 2, 5, 11, 12, 21, 22, 25, 101, 111, 112];
        let en = counts.map(|n| Lang::EnUs.plural(n));
        assert_eq!(
            en,
            [
                Many, One, Many, Many, Many, Many, Many, Many, Many, Many, Many, Many
            ]
        );
        let ru = counts.map(|n| Lang::RuRu.plural(n));
        assert_eq!(
            ru,
            [
                Many, One, Few, Many, Many, Many, One, Few, Many, One, Many, Many
            ]
        );
        assert_eq!(Lang::RuRu.plural(14), Many);
        assert_eq!(Lang::RuRu.plural(24), Few);
        assert_eq!((-21i32).count(), 21);
        assert_eq!(usize::MAX.count(), usize::MAX as u64);
    }

    #[test]
    fn trn_fills_the_english_form() {
        assert_eq!(active(), Lang::EnUs);
        assert_eq!(trn!("clickgui.mods.modules", 1usize), "1 module");
        assert_eq!(trn!("clickgui.mods.modules", 3u32), "3 modules");
        assert_eq!(
            trn!("clickgui.explore.listing", 0, "today"),
            "0 mods, updated today"
        );
    }

    #[test]
    fn codes_map_on_the_primary_subtag() {
        let ru = if cfg!(feature = "multi_language") {
            Lang::RuRu
        } else {
            Lang::EnUs
        };
        assert_eq!(Lang::from_code("ru_ru"), Lang::RuRu);
        assert_eq!(Lang::from_code("ru-RU"), Lang::RuRu);
        assert_eq!(Lang::from_code("RU"), Lang::RuRu);
        assert_eq!(Lang::from_code("en_us"), Lang::EnUs);
        assert_eq!(Lang::from_code("de_de"), Lang::EnUs);
        assert_eq!(Lang::from_code(""), Lang::EnUs);
        assert!(supported("en_gb"));
        assert!(!supported("de_de"));
        assert_eq!(supported("ru_ua"), ru == Lang::RuRu);
    }
}
