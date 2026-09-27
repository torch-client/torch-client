#[derive(Debug, Clone, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

const EMPTY: &[Json] = &[];

impl Json {
    pub fn parse(src: &str) -> Option<Json> {
        let b = src.as_bytes();
        let mut i = 0usize;
        let v = parse_value(b, &mut i, 0)?;
        skip_ws(b, &mut i);
        Some(v)
    }

    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Obj(fields) => fields.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn arr(&self) -> &[Json] {
        match self {
            Json::Arr(items) => items,
            _ => EMPTY,
        }
    }

    pub fn idx(&self, i: usize) -> Option<&Json> {
        self.arr().get(i)
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_f32(&self) -> Option<f32> {
        match self {
            Json::Num(n) => Some(*n as f32),
            _ => None,
        }
    }

    pub fn as_i32(&self) -> Option<i32> {
        match self {
            Json::Num(n) => Some(*n as i32),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Json::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_vec3(&self) -> Option<[f32; 3]> {
        let a = self.arr();
        if a.len() < 3 {
            return None;
        }
        Some([a[0].as_f32()?, a[1].as_f32()?, a[2].as_f32()?])
    }

    pub fn as_vec4(&self) -> Option<[f32; 4]> {
        let a = self.arr();
        if a.len() < 4 {
            return None;
        }
        Some([
            a[0].as_f32()?,
            a[1].as_f32()?,
            a[2].as_f32()?,
            a[3].as_f32()?,
        ])
    }
}

fn skip_ws(b: &[u8], i: &mut usize) {
    while *i < b.len() && matches!(b[*i], b' ' | b'\t' | b'\r' | b'\n') {
        *i += 1;
    }
}

fn parse_value(b: &[u8], i: &mut usize, depth: u32) -> Option<Json> {
    if depth > 64 {
        return None;
    }
    skip_ws(b, i);
    match *b.get(*i)? {
        b'{' => {
            *i += 1;
            let mut fields = Vec::new();
            skip_ws(b, i);
            if *b.get(*i)? == b'}' {
                *i += 1;
                return Some(Json::Obj(fields));
            }
            loop {
                skip_ws(b, i);
                let key = parse_string(b, i)?;
                skip_ws(b, i);
                if *b.get(*i)? != b':' {
                    return None;
                }
                *i += 1;
                let value = parse_value(b, i, depth + 1)?;
                fields.push((key, value));
                skip_ws(b, i);
                match *b.get(*i)? {
                    b',' => *i += 1,
                    b'}' => {
                        *i += 1;
                        return Some(Json::Obj(fields));
                    }
                    _ => return None,
                }
            }
        }
        b'[' => {
            *i += 1;
            let mut items = Vec::new();
            skip_ws(b, i);
            if *b.get(*i)? == b']' {
                *i += 1;
                return Some(Json::Arr(items));
            }
            loop {
                items.push(parse_value(b, i, depth + 1)?);
                skip_ws(b, i);
                match *b.get(*i)? {
                    b',' => *i += 1,
                    b']' => {
                        *i += 1;
                        return Some(Json::Arr(items));
                    }
                    _ => return None,
                }
            }
        }
        b'"' => Some(Json::Str(parse_string(b, i)?)),
        b't' => lit(b, i, b"true", Json::Bool(true)),
        b'f' => lit(b, i, b"false", Json::Bool(false)),
        b'n' => lit(b, i, b"null", Json::Null),
        _ => parse_number(b, i),
    }
}

fn lit(b: &[u8], i: &mut usize, word: &[u8], out: Json) -> Option<Json> {
    if b.len() >= *i + word.len() && &b[*i..*i + word.len()] == word {
        *i += word.len();
        Some(out)
    } else {
        None
    }
}

fn parse_number(b: &[u8], i: &mut usize) -> Option<Json> {
    let start = *i;
    if *b.get(*i)? == b'-' || *b.get(*i)? == b'+' {
        *i += 1;
    }
    while *i < b.len() && matches!(b[*i], b'0'..=b'9' | b'.' | b'e' | b'E' | b'-' | b'+') {
        *i += 1;
    }
    if start == *i {
        return None;
    }
    std::str::from_utf8(&b[start..*i])
        .ok()?
        .parse::<f64>()
        .ok()
        .map(Json::Num)
}

fn parse_string(b: &[u8], i: &mut usize) -> Option<String> {
    if *b.get(*i)? != b'"' {
        return None;
    }
    *i += 1;
    let mut out = String::new();
    loop {
        let c = *b.get(*i)?;
        *i += 1;
        match c {
            b'"' => return Some(out),
            b'\\' => {
                let e = *b.get(*i)?;
                *i += 1;
                match e {
                    b'"' => out.push('"'),
                    b'\\' => out.push('\\'),
                    b'/' => out.push('/'),
                    b'b' => out.push('\u{8}'),
                    b'f' => out.push('\u{c}'),
                    b'n' => out.push('\n'),
                    b'r' => out.push('\r'),
                    b't' => out.push('\t'),
                    b'u' => {
                        let hex = std::str::from_utf8(b.get(*i..*i + 4)?).ok()?;
                        let code = u32::from_str_radix(hex, 16).ok()?;
                        *i += 4;
                        out.push(char::from_u32(code).unwrap_or('\u{fffd}'));
                    }
                    _ => return None,
                }
            }
            _ => {
                let start = *i - 1;
                let len = utf8_len(c);
                let bytes = b.get(start..start + len)?;
                out.push_str(std::str::from_utf8(bytes).ok()?);
                *i = start + len;
            }
        }
    }
}

fn utf8_len(first: u8) -> usize {
    match first {
        0x00..=0x7f => 1,
        0xc0..=0xdf => 2,
        0xe0..=0xef => 3,
        _ => 4,
    }
}

#[cfg(test)]
mod tests {
    use super::Json;

    #[test]
    fn parses_a_model_shaped_document() {
        let src = r#"{"parent":"block/cube","x":-1.5e1,"ok":true,
                      "textures":{"all":"block/stone"},
                      "elements":[{"from":[0,0,0],"to":[16,16,16]}]}"#;
        let j = Json::parse(src).expect("parse");
        assert_eq!(j.get("parent").unwrap().as_str(), Some("block/cube"));
        assert_eq!(j.get("x").unwrap().as_f32(), Some(-15.0));
        assert_eq!(j.get("ok").unwrap().as_bool(), Some(true));
        assert_eq!(
            j.get("textures").unwrap().get("all").unwrap().as_str(),
            Some("block/stone")
        );
        let e = &j.get("elements").unwrap().arr()[0];
        assert_eq!(e.get("to").unwrap().as_vec3(), Some([16.0, 16.0, 16.0]));
    }
}
