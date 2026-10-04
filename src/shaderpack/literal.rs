pub(crate) fn int(text: &str) -> Option<i64> {
    let text = text.trim().trim_end_matches(['u', 'U']);
    match text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        Some(hex) => i64::from_str_radix(hex, 16).ok(),
        None => text.parse().ok(),
    }
}

pub(crate) fn float(text: &str) -> Option<f32> {
    text.trim().trim_end_matches(['f', 'F']).parse().ok()
}

pub(crate) fn constructor_args<'a>(value: &'a str, ty: &str) -> Option<Vec<&'a str>> {
    let inner = value
        .trim()
        .strip_prefix(ty)?
        .trim_start()
        .strip_prefix('(')?
        .strip_suffix(')')?;
    Some(inner.split(',').map(str::trim).collect())
}

pub(crate) fn broadcast<T: Copy, const N: usize>(values: Vec<T>) -> Option<[T; N]> {
    match values.len() {
        1 => Some([values[0]; N]),
        _ => values.try_into().ok(),
    }
}

pub(crate) fn const_declaration(line: &str) -> Option<(&str, &str, &str)> {
    let rest = line.trim().strip_prefix("const")?;
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }
    let (ty, rest) = rest.trim_start().split_once(char::is_whitespace)?;
    let (name, rest) = rest.trim_start().split_once('=')?;
    let value = rest.split(';').next()?.trim();
    Some((ty, name.trim(), value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literals_read_as_glsl_writes_them() {
        assert_eq!(int("0x10u"), Some(16));
        assert_eq!(int("-3"), Some(-3));
        assert_eq!(int("1.0"), None);
        assert_eq!(float("2.5f"), Some(2.5));
        assert_eq!(float("1e-3"), Some(0.001));
        assert_eq!(
            constructor_args("vec4(1.0, 0.5)", "vec4"),
            Some(vec!["1.0", "0.5"])
        );
        assert_eq!(constructor_args("vec3(1.0)", "vec4"), None);
        assert_eq!(broadcast::<_, 3>(vec![7]), Some([7; 3]));
        assert_eq!(broadcast::<_, 2>(vec![1, 2, 3]), None);
        assert_eq!(
            const_declaration("const int a = 3; // x"),
            Some(("int", "a", "3"))
        );
        assert_eq!(const_declaration("constant x = 1;"), None);
    }
}
