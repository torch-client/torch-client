use super::args::{Arg, ArgKind, ArgValue, Tok, bind, rest, tokens};
use super::table::{Decl, ScriptCommand, Table};
use super::*;
use crate::modules::registry::ModuleIx;
use crate::play::commands::{Options, Source, update_command_info};

fn texts(raw: &str) -> Vec<String> {
    tokens(raw).unwrap().into_iter().map(|t| t.text).collect()
}

#[test]
fn tokens_split_on_spaces_and_honour_quotes() {
    assert_eq!(texts("  a  bb c "), ["a", "bb", "c"]);
    assert_eq!(
        texts(r#"say "two words" "q\"uote""#),
        ["say", "two words", r#"q"uote"#]
    );
    assert_eq!(tokens("a \"open").unwrap_err(), "Unclosed quoted string");
    assert!(tokens(r#""bad \x""#).is_err());
    assert_eq!(
        tokens("x  y").unwrap()[1],
        Tok {
            text: "y".into(),
            start: 3
        }
    );
}

#[test]
fn rest_is_the_line_as_typed() {
    let raw = r#"aimassist  fov "60" left "#;
    let toks = tokens(raw).unwrap();
    assert_eq!(rest(raw, &toks, 2), r#""60" left"#);
    assert_eq!(rest(raw, &toks, 9), "");
}

fn arg(name: &'static str, kind: ArgKind, optional: bool) -> Arg {
    Arg {
        name,
        kind,
        optional,
        suggest: Vec::new(),
    }
}

#[test]
fn bind_types_and_counts() {
    let args = [
        arg("n", ArgKind::Int, false),
        arg("x", ArgKind::Number, false),
        arg("b", ArgKind::Bool, true),
    ];
    assert_eq!(
        bind(&args, "3 2.5 true").unwrap(),
        vec![
            ("n", ArgValue::Int(3)),
            ("x", ArgValue::Num(2.5)),
            ("b", ArgValue::Bool(true))
        ]
    );
    assert_eq!(bind(&args, "3 2.5").unwrap().len(), 2);
    assert_eq!(bind(&args, "3").unwrap_err(), "Missing <x>");
    assert!(bind(&args, "three 1").unwrap_err().contains("<n>"));
    assert!(bind(&args, "1 inf").is_err());
    assert!(bind(&args, "1 1 yes").is_err());
    assert!(bind(&args, "1 1 true extra").unwrap_err().contains("extra"));
}

#[test]
fn a_string_argument_takes_the_rest() {
    let args = [
        arg("op", ArgKind::Word, false),
        arg("text", ArgKind::String, true),
    ];
    assert_eq!(
        bind(&args, r#""add" my  home "#).unwrap(),
        vec![
            ("op", ArgValue::Str("add".into())),
            ("text", ArgValue::Str("my  home".into()))
        ]
    );
    assert_eq!(bind(&args, "list").unwrap().len(), 1);
}

#[test]
fn prefixes_that_would_take_chat_are_refused() {
    assert!(check_prefix(".").is_ok());
    assert!(check_prefix("!!").is_ok());
    assert!(check_prefix("").is_err());
    assert!(check_prefix("/").is_err());
    assert!(check_prefix("a").is_err());
    assert!(check_prefix("7").is_err());
    assert!(check_prefix(". ").is_err());
    assert!(check_prefix("-----").is_err());
}

fn script(mod_id: &'static str, name: &'static str, aliases: Vec<&'static str>) -> ScriptCommand {
    ScriptCommand {
        mod_id,
        module: ModuleIx(9999),
        decl: Decl {
            name,
            aliases,
            tip: "",
            args: Vec::new(),
        },
    }
}

#[test]
fn built_ins_win_and_every_mod_command_keeps_its_qualified_name() {
    let (t, clashes) = Table::with_scripts(vec![
        script("a", "wp", vec!["t"]),
        script("b", "wp", vec!["waypoint"]),
    ]);
    assert_eq!(clashes.len(), 2, "{clashes:?}");
    assert_eq!(t.find("t").map(|c| c.name()), Some("toggle"));
    let wp = t.find("wp").unwrap();
    assert_eq!(wp.qualified.as_deref(), Some("a:wp"));
    assert_eq!(t.find("b:wp").unwrap().names, ["waypoint"]);
    assert_eq!(
        t.find("waypoint").unwrap().qualified.as_deref(),
        Some("b:wp")
    );
    assert!(t.find("nope").is_none());
}

fn complete(t: &Table, line: &str) -> Vec<String> {
    let tree = super::tree::build(t);
    let options = Options {
        lead: '.',
        ..Options::CHAT
    };
    let info = update_command_info(
        Some(&tree),
        line,
        line.chars().count(),
        &Source::default(),
        options,
        None,
    );
    assert!(info.ask_server.is_none(), "{line} asked the server");
    info.suggestions.list.into_iter().map(|s| s.text).collect()
}

#[test]
fn the_tree_completes_commands_modules_settings_and_options() {
    let (t, _) = Table::with_scripts(Vec::new());
    assert!(complete(&t, ".to").contains(&"toggle".to_string()));
    assert!(complete(&t, ".toggle aim").contains(&"aimassist".to_string()));
    assert_eq!(complete(&t, ".set nametags si"), ["sizing"]);
    let sizing = complete(&t, ".set nametags sizing ");
    assert!(
        !sizing.is_empty() && sizing.iter().all(|s| s.chars().all(|c| !c.is_uppercase())),
        "{sizing:?}"
    );
    assert_eq!(complete(&t, ".profile b"), ["blatant"]);
}

#[test]
fn follow_completes_tab_list_names_and_parses_bare() {
    let (t, _) = Table::with_scripts(Vec::new());
    let tree = super::tree::build(&t);
    let options = Options {
        lead: '.',
        ..Options::CHAT
    };
    let source = Source {
        player_names: vec!["Alex".to_string(), "Steve".to_string()],
        ..Source::default()
    };
    let line = ".follow St";
    let info = update_command_info(
        Some(&tree),
        line,
        line.chars().count(),
        &source,
        options,
        None,
    );
    assert!(info.ask_server.is_none(), "{line} asked the server");
    let names: Vec<String> = info.suggestions.list.into_iter().map(|s| s.text).collect();
    assert_eq!(names, ["Steve"]);
    for line in [".follow", ".follow Steve"] {
        let info = update_command_info(
            Some(&tree),
            line,
            line.chars().count(),
            &source,
            options,
            None,
        );
        let red = info.usage.iter().any(|(_, error)| *error);
        assert!(!red, "{line}: {:?}", info.usage);
    }
}

#[test]
fn a_mod_commands_suggest_list_completes_and_any_value_parses() {
    let mut s = script("m", "wp", Vec::new());
    s.decl.args = vec![
        Arg {
            name: "op",
            kind: ArgKind::Word,
            optional: false,
            suggest: vec!["add", "del"],
        },
        arg("label", ArgKind::String, true),
    ];
    let (t, _) = Table::with_scripts(vec![s]);
    assert_eq!(complete(&t, ".wp a"), ["add"]);
    let tree = super::tree::build(&t);
    let options = Options {
        lead: '.',
        ..Options::CHAT
    };
    for line in [".wp add home", ".wp other", ".m:wp del"] {
        let info = update_command_info(
            Some(&tree),
            line,
            line.chars().count(),
            &Source::default(),
            options,
            None,
        );
        let red = info.usage.iter().any(|(_, error)| *error);
        assert!(!red, "{line}: {:?}", info.usage);
    }
}
