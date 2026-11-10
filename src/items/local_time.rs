use std::sync::Mutex;

const UPDATE_INTERVAL_MS: i64 = 1000;

#[derive(Clone, Debug, PartialEq)]
enum Token {
    Literal(Box<str>),
    Year(u8),
    Month(u8),
    Day(u8),
    DayOfYear(u8),
    Weekday(u8),
    AmPm,
    Hour23(u8),
    Hour24(u8),
    Hour11(u8),
    Hour12(u8),
    Minute(u8),
    Second(u8),
    Fraction(u8),
    MillisInDay(u8),
    ZoneRfc(u8),
    ZoneIso { count: u8, z: bool },
}

struct Format {
    key: Box<str>,
    tokens: Vec<Token>,
    offset: i64,
    next_update: i64,
    last: String,
}

static FORMATS: Mutex<Vec<Format>> = Mutex::new(Vec::new());

pub fn intern(pattern: &str, locale: &str, time_zone: Option<&str>) -> Option<u16> {
    let key: Box<str> = format!("{pattern}\0{locale}\0{}", time_zone.unwrap_or("")).into();
    let mut formats = FORMATS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(at) = formats.iter().position(|f| f.key == key) {
        return u16::try_from(at).ok();
    }
    let offset = match time_zone {
        Some(zone) => zone_offset(zone)?,
        None => 0,
    };
    let tokens = parse(pattern, english(locale))?;
    let at = u16::try_from(formats.len()).ok()?;
    formats.push(Format {
        key,
        tokens,
        offset,
        next_update: i64::MIN,
        last: String::new(),
    });
    Some(at)
}

pub fn pick<T>(index: u16, f: impl FnOnce(&str) -> T) -> T {
    let now = now_ms();
    let mut formats = FORMATS.lock().unwrap_or_else(|e| e.into_inner());
    let Some(format) = formats.get_mut(usize::from(index)) else {
        return f("");
    };
    if now > format.next_update {
        format.last = render(&format.tokens, now + format.offset * 1000, format.offset);
        format.next_update = now + UPDATE_INTERVAL_MS;
    }
    f(&format.last)
}

fn now_ms() -> i64 {
    crate::platform::time::SystemTime::now()
        .duration_since(crate::platform::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

fn english(locale: &str) -> bool {
    let lower = locale.to_ascii_lowercase();
    lower == "en" || lower.starts_with("en_") || lower.starts_with("en-")
}

fn zone_offset(id: &str) -> Option<i64> {
    const UTC: &[&str] = &[
        "UTC",
        "GMT",
        "UCT",
        "Zulu",
        "Universal",
        "Greenwich",
        "GMT0",
    ];
    let bare = id.strip_prefix("Etc/").unwrap_or(id);
    if UTC.contains(&bare) {
        return Some(0);
    }
    if let Some(rest) = id.strip_prefix("Etc/GMT") {
        let (sign, digits) = signed(rest)?;
        let hours: i64 = digits.parse().ok().filter(|h| (0..=14).contains(h))?;
        return Some(-sign * hours * 3600);
    }
    let rest = id.strip_prefix("GMT")?;
    let (sign, digits) = signed(rest)?;
    let (hours, minutes) = match digits.split_once(':') {
        Some((h, m)) if (1..=2).contains(&h.len()) && m.len() == 2 => (h, m),
        Some(_) => return None,
        None if digits.len() <= 2 => (digits, "0"),
        None if digits.len() == 4 => digits.split_at(2),
        None => return None,
    };
    if !hours
        .bytes()
        .chain(minutes.bytes())
        .all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let hours: i64 = hours.parse().ok().filter(|h| *h <= 23)?;
    let minutes: i64 = minutes.parse().ok().filter(|m| *m <= 59)?;
    Some(sign * (hours * 3600 + minutes * 60))
}

fn signed(s: &str) -> Option<(i64, &str)> {
    let rest = s
        .strip_prefix('+')
        .map(|r| (1, r))
        .or_else(|| s.strip_prefix('-').map(|r| (-1, r)))?;
    (!rest.1.is_empty()).then_some(rest)
}

fn parse(pattern: &str, english: bool) -> Option<Vec<Token>> {
    let chars: Vec<char> = pattern.chars().collect();
    let mut out = Vec::new();
    let mut literal = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\'' {
            if chars.get(i + 1) == Some(&'\'') {
                literal.push('\'');
                i += 2;
                continue;
            }
            i += 1;
            loop {
                match chars.get(i) {
                    None => return None,
                    Some('\'') if chars.get(i + 1) == Some(&'\'') => {
                        literal.push('\'');
                        i += 2;
                    }
                    Some('\'') => {
                        i += 1;
                        break;
                    }
                    Some(&q) => {
                        literal.push(q);
                        i += 1;
                    }
                }
            }
            continue;
        }
        if !c.is_ascii_alphabetic() {
            literal.push(c);
            i += 1;
            continue;
        }
        let mut run = 1;
        while chars.get(i + run) == Some(&c) {
            run += 1;
        }
        i += run;
        if !literal.is_empty() {
            out.push(Token::Literal(std::mem::take(&mut literal).into()));
        }
        let n = u8::try_from(run).unwrap_or(u8::MAX);
        out.push(match c {
            'y' => Token::Year(n),
            'M' | 'L' if n <= 2 || english => Token::Month(n),
            'd' => Token::Day(n),
            'D' => Token::DayOfYear(n),
            'E' if english => Token::Weekday(n),
            'a' => Token::AmPm,
            'H' => Token::Hour23(n),
            'k' => Token::Hour24(n),
            'K' => Token::Hour11(n),
            'h' => Token::Hour12(n),
            'm' => Token::Minute(n),
            's' => Token::Second(n),
            'S' => Token::Fraction(n),
            'A' => Token::MillisInDay(n),
            'Z' if n <= 5 => Token::ZoneRfc(n),
            'X' if n <= 3 => Token::ZoneIso { count: n, z: true },
            'x' if n <= 3 => Token::ZoneIso { count: n, z: false },
            _ => return None,
        });
    }
    if !literal.is_empty() {
        out.push(Token::Literal(literal.into()));
    }
    Some(out)
}

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
const WEEKDAYS: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];

fn render(tokens: &[Token], local_ms: i64, offset: i64) -> String {
    use std::fmt::Write;

    let days = local_ms.div_euclid(86_400_000);
    let ms_of_day = local_ms.rem_euclid(86_400_000);
    let (year, month, day) = crate::util::calendar::civil_from_days(days);
    let hour = ms_of_day / 3_600_000;
    let minute = ms_of_day / 60_000 % 60;
    let second = ms_of_day / 1000 % 60;
    let millis = ms_of_day % 1000;
    let weekday = (days + 4).rem_euclid(7) as usize;
    let day_of_year = day_of_year(year, month, day);

    let mut out = String::new();
    let pad = |out: &mut String, value: i64, n: u8| {
        let _ = write!(out, "{value:0width$}", width = usize::from(n));
    };
    for token in tokens {
        match *token {
            Token::Literal(ref text) => out.push_str(text),
            Token::Year(2) => pad(&mut out, year.rem_euclid(100), 2),
            Token::Year(n) => pad(&mut out, year, n),
            Token::Month(n) if n <= 2 => pad(&mut out, i64::from(month), n),
            Token::Month(n) => {
                let name = MONTHS[month as usize - 1];
                match n {
                    3 => out.push_str(&name[..3]),
                    4 => out.push_str(name),
                    _ => out.push_str(&name[..1]),
                }
            }
            Token::Day(n) => pad(&mut out, i64::from(day), n),
            Token::DayOfYear(n) => pad(&mut out, day_of_year, n),
            Token::Weekday(n) => {
                let name = WEEKDAYS[weekday];
                match n {
                    1..=3 => out.push_str(&name[..3]),
                    4 => out.push_str(name),
                    5 => out.push_str(&name[..1]),
                    _ => out.push_str(&name[..2]),
                }
            }
            Token::AmPm => out.push_str(if hour < 12 { "AM" } else { "PM" }),
            Token::Hour23(n) => pad(&mut out, hour, n),
            Token::Hour24(n) => pad(&mut out, if hour == 0 { 24 } else { hour }, n),
            Token::Hour11(n) => pad(&mut out, hour % 12, n),
            Token::Hour12(n) => pad(&mut out, if hour % 12 == 0 { 12 } else { hour % 12 }, n),
            Token::Minute(n) => pad(&mut out, minute, n),
            Token::Second(n) => pad(&mut out, second, n),
            Token::Fraction(n) if n <= 3 => {
                pad(&mut out, millis / 10i64.pow(u32::from(3 - n)), n);
            }
            Token::Fraction(n) => {
                pad(&mut out, millis, 3);
                out.extend(std::iter::repeat_n('0', usize::from(n - 3)));
            }
            Token::MillisInDay(n) => pad(&mut out, ms_of_day, n),
            Token::ZoneRfc(n) => match n {
                4 if offset == 0 => out.push_str("GMT"),
                4 => {
                    out.push_str("GMT");
                    zone(&mut out, offset, true);
                }
                5 if offset == 0 => out.push('Z'),
                5 => zone(&mut out, offset, true),
                _ => zone(&mut out, offset, false),
            },
            Token::ZoneIso { count, z } => {
                if z && offset == 0 {
                    out.push('Z');
                } else if count == 1 && offset % 3600 == 0 {
                    let sign = if offset < 0 { '-' } else { '+' };
                    let _ = write!(out, "{sign}{:02}", offset.abs() / 3600);
                } else {
                    zone(&mut out, offset, count == 3);
                }
            }
        }
    }
    out
}

fn zone(out: &mut String, offset: i64, colon: bool) {
    use std::fmt::Write;
    let sign = if offset < 0 { '-' } else { '+' };
    let (hours, minutes) = (offset.abs() / 3600, offset.abs() / 60 % 60);
    let separator = if colon { ":" } else { "" };
    let _ = write!(out, "{sign}{hours:02}{separator}{minutes:02}");
}

fn day_of_year(year: i64, month: u32, day: u32) -> i64 {
    const BEFORE: [i64; 12] = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    BEFORE[month as usize - 1] + i64::from(day) + i64::from(leap && month > 2)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHRISTMAS_EVE: i64 = 1_798_117_509_123;

    #[test]
    fn the_chest_pattern_reads_month_and_day() {
        let tokens = parse("MM-dd", false).unwrap();
        assert_eq!(render(&tokens, CHRISTMAS_EVE, 0), "12-24");
    }

    #[test]
    fn fields_pad_and_quote_as_simple_date_format_does() {
        let tokens = parse("yyyy.MM.dd 'at' HH:mm:ss.SSS ''yy", true).unwrap();
        assert_eq!(
            render(&tokens, CHRISTMAS_EVE, 0),
            "2026.12.24 at 13:05:09.123 '26"
        );
        let names = parse("EEE, MMM d h a", true).unwrap();
        assert_eq!(render(&names, CHRISTMAS_EVE, 0), "Thu, Dec 24 1 PM");
        assert!(parse("MMM", false).is_none());
        assert!(parse("w", true).is_none());
    }

    #[test]
    fn fixed_zones_parse_and_shift() {
        assert_eq!(zone_offset("UTC"), Some(0));
        assert_eq!(zone_offset("Etc/GMT+5"), Some(-5 * 3600));
        assert_eq!(zone_offset("GMT+05:30"), Some(5 * 3600 + 30 * 60));
        assert_eq!(zone_offset("GMT-8"), Some(-8 * 3600));
        assert_eq!(zone_offset("Europe/Paris"), None);
        let tokens = parse("HH:mm Z", false).unwrap();
        let offset = 5 * 3600 + 30 * 60;
        assert_eq!(
            render(&tokens, CHRISTMAS_EVE + offset * 1000, offset),
            "18:35 +0530"
        );
    }

    #[test]
    fn day_of_year_counts_the_leap_day() {
        assert_eq!(day_of_year(2024, 3, 1), 61);
        assert_eq!(day_of_year(2026, 3, 1), 60);
        assert_eq!(day_of_year(2026, 12, 24), 358);
    }
}
