use std::collections::{HashMap, HashSet};

const CONST_OPTIONS: &[&str] = &[
    "shadowMapResolution",
    "shadowDistance",
    "shadowDistanceRenderMul",
    "shadowIntervalSize",
    "shadowMapFov",
    "shadowNearPlane",
    "shadowFarPlane",
    "entityShadowDistanceMul",
    "voxelDistance",
    "sunPathRotation",
    "ambientOcclusionLevel",
    "superSamplingLevel",
    "noiseTextureResolution",
    "wetnessHalflife",
    "drynessHalflife",
    "eyeBrightnessHalflife",
    "centerDepthHalflife",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Boolean { default: bool },
    Value { values: Vec<String>, default: usize },
}

#[derive(Clone, Debug)]
pub(crate) struct Opt {
    pub(crate) name: String,
    pub(crate) kind: Kind,
    pub(crate) is_const: bool,
}

impl Opt {
    pub(crate) fn default_value(&self) -> &str {
        match &self.kind {
            Kind::Boolean { default: true } => "true",
            Kind::Boolean { default: false } => "false",
            Kind::Value { values, default } => &values[*default],
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct Options {
    all: Vec<Opt>,
    index: HashMap<String, usize>,
}

impl Options {
    pub(crate) fn get(&self, name: &str) -> Option<&Opt> {
        self.index.get(name).map(|at| &self.all[*at])
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &Opt> {
        self.all.iter()
    }

    pub(crate) fn len(&self) -> usize {
        self.all.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.all.is_empty()
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Values {
    overrides: HashMap<String, String>,
}

impl Values {
    pub(crate) fn is_empty(&self) -> bool {
        self.overrides.is_empty()
    }

    pub(crate) fn get<'a>(&'a self, option: &'a Opt) -> &'a str {
        self.overrides
            .get(&option.name)
            .map_or_else(|| option.default_value(), String::as_str)
    }

    pub(crate) fn is_default(&self, option: &Opt) -> bool {
        !self.overrides.contains_key(&option.name)
    }

    pub(crate) fn set(&mut self, option: &Opt, value: &str) {
        if value == option.default_value() {
            self.overrides.remove(&option.name);
        } else {
            self.overrides.insert(option.name.clone(), value.to_owned());
        }
    }

    pub(crate) fn reset(&mut self, option: &Opt) {
        self.overrides.remove(&option.name);
    }

    pub(crate) fn reset_all(&mut self) {
        self.overrides.clear();
    }

    pub(crate) fn load(text: &str, options: &Options) -> Values {
        let mut values = Values::default();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((name, value)) = line.split_once('=') else {
                continue;
            };
            if let Some(option) = options.get(name.trim()) {
                values.set(option, value.trim());
            }
        }
        values
    }

    pub(crate) fn save(&self) -> String {
        let mut lines: Vec<String> = self
            .overrides
            .iter()
            .map(|(name, value)| format!("{name}={value}"))
            .collect();
        lines.sort();
        lines.join("\n")
    }
}

struct Decl<'a> {
    name: &'a str,
    value: Option<&'a str>,
    values: Vec<&'a str>,
    enabled: bool,
    is_const: bool,
}

pub(crate) fn discover<'a>(texts: impl Iterator<Item = &'a str> + Clone) -> Options {
    let mut tested: HashSet<&str> = HashSet::new();
    for text in texts.clone() {
        for line in text.lines() {
            if let Some(name) = conditional_name(line) {
                tested.insert(name);
            }
        }
    }

    let mut options = Options::default();
    let mut ambiguous: HashSet<String> = HashSet::new();

    for text in texts {
        for line in text.lines() {
            let Some(decl) = declaration(line) else {
                continue;
            };
            let kind = match decl.value {
                None if !tested.contains(decl.name) => continue,
                None => Kind::Boolean {
                    default: decl.enabled,
                },
                Some(value) => {
                    if decl.values.is_empty() {
                        continue;
                    }
                    let mut values: Vec<String> =
                        decl.values.iter().map(|v| (*v).to_owned()).collect();
                    let default = values.iter().position(|v| v == value).unwrap_or_else(|| {
                        values.insert(0, value.to_owned());
                        0
                    });
                    Kind::Value { values, default }
                }
            };

            match options.index.get(decl.name) {
                Some(at) if options.all[*at].kind != kind => {
                    ambiguous.insert(decl.name.to_owned());
                }
                Some(_) => {}
                None => {
                    options
                        .index
                        .insert(decl.name.to_owned(), options.all.len());
                    options.all.push(Opt {
                        name: decl.name.to_owned(),
                        kind,
                        is_const: decl.is_const,
                    });
                }
            }
        }
    }

    if !ambiguous.is_empty() {
        options.all.retain(|opt| !ambiguous.contains(&opt.name));
        options.index = options
            .all
            .iter()
            .enumerate()
            .map(|(at, opt)| (opt.name.clone(), at))
            .collect();
    }
    options
}

pub(crate) fn apply(text: &str, options: &Options, values: &Values) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.lines() {
        match declaration(line).and_then(|decl| {
            let option = options.get(decl.name)?;
            Some((decl, option, values.get(option)))
        }) {
            Some((decl, option, value)) if value != shipped(&decl, option) => {
                out.push_str(&rewrite(line, &decl, value));
            }
            _ => out.push_str(line),
        }
        out.push('\n');
    }
    out
}

fn shipped<'a>(decl: &'a Decl<'a>, option: &'a Opt) -> &'a str {
    match (&option.kind, decl.value) {
        (Kind::Boolean { .. }, _) => {
            if decl.enabled {
                "true"
            } else {
                "false"
            }
        }
        (_, Some(value)) => value,
        (_, None) => "",
    }
}

fn rewrite(line: &str, decl: &Decl<'_>, value: &str) -> String {
    match decl.value {
        None => match value {
            "true" => uncomment(line),
            _ => comment(line),
        },
        Some(old) => {
            let at = value_span(line, decl, old);
            format!("{}{value}{}", &line[..at.0], &line[at.1..])
        }
    }
}

fn value_span(line: &str, decl: &Decl<'_>, old: &str) -> (usize, usize) {
    let from = line.find(decl.name).map_or(0, |at| at + decl.name.len());
    match line[from..].find(old) {
        Some(at) => (from + at, from + at + old.len()),
        None => (line.len(), line.len()),
    }
}

fn comment(line: &str) -> String {
    let at = line.len() - line.trim_start().len();
    format!("{}//{}", &line[..at], &line[at..])
}

fn uncomment(line: &str) -> String {
    let trimmed = line.trim_start();
    let at = line.len() - trimmed.len();
    match trimmed.strip_prefix("//") {
        Some(rest) => format!("{}{}", &line[..at], rest.trim_start()),
        None => line.to_owned(),
    }
}

fn conditional_name(line: &str) -> Option<&str> {
    let rest = line.trim_start().strip_prefix('#')?.trim_start();
    let rest = rest
        .strip_prefix("ifdef")
        .or_else(|| rest.strip_prefix("ifndef"))?;
    let name = rest.trim().split_whitespace().next()?;
    name.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_')
        .then_some(name)
}

fn declaration(line: &str) -> Option<Decl<'_>> {
    let (code, comment) = split_comment(line);
    let values = allowed(comment);
    let trimmed = code.trim_start();

    let (enabled, body) = match trimmed.strip_prefix("//") {
        Some(rest) => (false, rest.trim_start()),
        None => (true, trimmed),
    };

    if let Some(rest) = body.strip_prefix('#') {
        let rest = rest.trim_start().strip_prefix("define")?;
        if !rest.starts_with(char::is_whitespace) {
            return None;
        }
        let mut parts = rest.trim().splitn(2, char::is_whitespace);
        let name = parts.next().filter(|n| is_identifier(n))?;
        let value = parts.next().map(str::trim).filter(|v| !v.is_empty());
        return Some(Decl {
            name,
            value,
            values,
            enabled,
            is_const: false,
        });
    }

    if !enabled {
        return None;
    }
    let rest = body.strip_prefix("const")?;
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }
    let (_type, rest) = rest.trim_start().split_once(char::is_whitespace)?;
    let (name, value) = rest.trim_start().split_once('=')?;
    let name = name.trim();
    if !CONST_OPTIONS.contains(&name) {
        return None;
    }
    Some(Decl {
        name,
        value: Some(value.trim().trim_end_matches(';').trim()),
        values,
        enabled: true,
        is_const: true,
    })
}

fn split_comment(line: &str) -> (&str, &str) {
    let body = line.trim_start();
    let offset = line.len() - body.len();
    let from = if body.starts_with("//") { 2 } else { 0 };
    match body[from..].find("//").map(|at| offset + from + at) {
        Some(at) => (&line[..at], &line[at + 2..]),
        None => match line.find("/*") {
            Some(at) => (&line[..at], &line[at + 2..]),
            None => (line, ""),
        },
    }
}

fn allowed(comment: &str) -> Vec<&str> {
    let open = comment.find('[').unwrap_or(comment.len());
    let Some(close) = comment.get(open..).and_then(|rest| rest.find(']')) else {
        return Vec::new();
    };
    comment[open + 1..open + close].split_whitespace().collect()
}

fn is_identifier(name: &str) -> bool {
    !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(text: &str) -> Options {
        discover([text].into_iter())
    }

    #[test]
    fn the_three_declaration_forms_are_read() {
        let options = found(
            "#ifdef A\n#endif\n#ifdef B\n#endif\n\
             #define A\n//#define B\n\
             #define V 14.25 //[15.00 14.25 13.00]\n\
             const int shadowMapResolution = 2048; //[1024 2048 4096]\n",
        );
        assert_eq!(options.len(), 4);
        assert_eq!(
            options.get("A").unwrap().kind,
            Kind::Boolean { default: true }
        );
        assert_eq!(
            options.get("B").unwrap().kind,
            Kind::Boolean { default: false }
        );
        assert_eq!(
            options.get("V").unwrap().kind,
            Kind::Value {
                values: ["15.00", "14.25", "13.00"].map(String::from).to_vec(),
                default: 1,
            }
        );
        assert!(options.get("shadowMapResolution").unwrap().is_const);
    }

    #[test]
    fn declarations_that_are_not_settings_are_skipped() {
        let options = found("#define PI 3.14159\n#define HELPER\nconst int unrelated = 4;\n");
        assert_eq!(options.len(), 0);
    }

    #[test]
    fn a_single_valued_option_survives() {
        let options = found("#define SOLAS_BY_SEPTONIOUS 1 //[1]\n");
        assert_eq!(options.len(), 1);
        assert_eq!(
            options.get("SOLAS_BY_SEPTONIOUS").unwrap().default_value(),
            "1"
        );
    }

    #[test]
    fn only_overrides_are_stored_and_saved() {
        let options = found("#ifdef A\n#endif\n#define A\n#define V 2 //[1 2 3]\n");
        let mut values = Values::default();
        let a = options.get("A").unwrap();
        let v = options.get("V").unwrap();

        assert!(values.is_empty());
        values.set(v, "3");
        values.set(a, "true");
        assert_eq!(values.save(), "V=3");
        assert!(values.is_default(a) && !values.is_default(v));

        values.set(v, "2");
        assert!(
            values.is_empty(),
            "a value back at its default is not stored"
        );
    }

    #[test]
    fn values_round_trip_through_the_document() {
        let options = found("#ifdef A\n#endif\n#define A\n#define V 2 //[1 2 3]\n");
        let mut values = Values::default();
        values.set(options.get("A").unwrap(), "false");
        values.set(options.get("V").unwrap(), "1");

        let reloaded = Values::load(&values.save(), &options);
        assert_eq!(reloaded.get(options.get("A").unwrap()), "false");
        assert_eq!(reloaded.get(options.get("V").unwrap()), "1");
    }

    #[test]
    fn applying_edits_only_what_it_must() {
        let text = "#ifdef A\n#endif\n\
                    #define A\n\
                    \t#define V 14.25 //[15.00 14.25 13.00]\n\
                    const int shadowMapResolution = 2048; //[1024 2048 4096]\n";
        let options = found(text);
        let mut values = Values::default();
        values.set(options.get("A").unwrap(), "false");
        values.set(options.get("V").unwrap(), "15.00");
        values.set(options.get("shadowMapResolution").unwrap(), "4096");

        let out = apply(text, &options, &values);
        assert!(out.contains("//#define A\n"), "{out}");
        assert!(
            out.contains("\t#define V 15.00 //[15.00 14.25 13.00]\n"),
            "{out}"
        );
        assert!(
            out.contains("const int shadowMapResolution = 4096; //[1024 2048 4096]\n"),
            "{out}"
        );

        values.set(options.get("V").unwrap(), "13.00");
        let again = apply(text, &options, &values);
        assert!(
            again.contains("#define V 13.00 //[15.00 14.25 13.00]\n"),
            "{again}"
        );
    }

    #[test]
    fn turning_a_boolean_back_on_removes_the_comment() {
        let text = "#ifdef B\n#endif\n  //#define B\n";
        let options = found(text);
        let mut values = Values::default();
        values.set(options.get("B").unwrap(), "true");
        assert!(apply(text, &options, &values).contains("  #define B\n"));
    }

    #[test]
    #[ignore = "needs a shader pack installed"]
    fn a_real_pack_declares_options_its_menu_can_name() {
        use super::super::{discover as packs, properties, source::Source};

        for pack in packs::list() {
            let at = packs::dir().join(&pack.name);
            let Ok(source) = Source::open(&at) else {
                continue;
            };

            let files: Vec<String> = source
                .files()
                .into_iter()
                .filter(|f| {
                    [".glsl", ".vsh", ".fsh", ".gsh", ".csh"]
                        .iter()
                        .any(|e| f.ends_with(e))
                })
                .collect();
            let texts: Vec<String> = files.iter().filter_map(|f| source.read_text(f)).collect();
            let options = discover(texts.iter().map(String::as_str));

            let (booleans, values) = options
                .iter()
                .partition::<Vec<_>, _>(|o| matches!(o.kind, Kind::Boolean { .. }));
            let consts = options.iter().filter(|o| o.is_const).count();
            println!(
                "{}: {} options ({} boolean, {} valued, {consts} const) from {} files",
                pack.name,
                options.len(),
                booleans.len(),
                values.len(),
                texts.len(),
            );

            let Some(text) = source.read_text("/shaders.properties") else {
                continue;
            };
            let props = properties::parse(&text, &mut super::super::features::base_defines());

            let mut missing: Vec<&str> = props
                .screens
                .values()
                .flatten()
                .filter_map(|e| match e {
                    properties::Element::Option(name) => Some(name.as_str()),
                    _ => None,
                })
                .filter(|name| options.get(name).is_none())
                .collect();
            missing.sort_unstable();
            missing.dedup();
            for name in missing.iter().take(10) {
                println!("    on a screen but not declared: {name}");
            }
            assert!(
                missing.is_empty(),
                "{}: {} names on screens are not declared options",
                pack.name,
                missing.len()
            );
        }
    }

    #[test]
    fn an_option_declared_two_ways_is_dropped() {
        let options = discover(
            [
                "#define V 1 //[1 2]",
                "#define V 1 //[1 2 3]",
                "#define W 1 //[1 2]",
            ]
            .into_iter(),
        );
        assert!(options.get("V").is_none());
        assert!(options.get("W").is_some());
    }
}
