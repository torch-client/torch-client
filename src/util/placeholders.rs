pub(crate) fn substitute(template: &str, args: &[String]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    let mut next = args.iter();
    while let Some(i) = rest.find('%') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let bytes = rest.as_bytes();
        let digits = bytes[1..].iter().take_while(|b| b.is_ascii_digit()).count();
        match (digits, bytes.get(1 + digits)) {
            (1.., Some(b'$')) if bytes.get(2 + digits) == Some(&b's') => {
                let n: usize = rest[1..1 + digits].parse().unwrap_or(0);
                match n.checked_sub(1).and_then(|i| args.get(i)) {
                    Some(a) => out.push_str(a),
                    None => out.push_str(&rest[..3 + digits]),
                }
                rest = &rest[3 + digits..];
            }
            (0, Some(b'%')) => {
                out.push('%');
                rest = &rest[2..];
            }
            (0, Some(b's')) => {
                match next.next() {
                    Some(a) => out.push_str(a),
                    None => out.push_str("%s"),
                }
                rest = &rest[2..];
            }
            _ => {
                out.push('%');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}
