use super::args::{Arg, ArgKind, ArgValue, Tok, bind, rest, tokens};
use super::builtin::{norm, pick, remapped};
use super::table::{Decl, ScriptCommand, Table};
use super::tree::{Names, for_line};
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
    let tree = super::tree::build(t, Names::English);
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
    let dia = complete(&t, ".mine dia");
    for name in ["diamond", "diamond_ore", "diamond_block"] {
        assert!(dia.contains(&name.to_string()), "{dia:?}");
    }
    let st = complete(&t, ".mine st");
    assert!(
        st.contains(&"stop".to_string()) && st.contains(&"stone".to_string()),
        "{st:?}"
    );
    assert_eq!(complete(&t, ".mine woo"), ["wood"]);
    assert!(complete(&t, ".mine lo").contains(&"log".to_string()));
}

#[test]
fn mine_parses_a_name_off_the_list() {
    let (t, _) = Table::with_scripts(Vec::new());
    let tree = super::tree::build(&t, Names::English);
    let options = Options {
        lead: '.',
        ..Options::CHAT
    };
    for line in [
        ".mine",
        ".mine wood",
        ".mine oak_log",
        ".mine minecraft:stone",
        ".mine redstone 40",
        ".mine wood 1",
        ".mine minecraft:stone 5",
    ] {
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
    for line in [".mine redstone 0", ".mine stop 2"] {
        let info = update_command_info(
            Some(&tree),
            line,
            line.chars().count(),
            &Source::default(),
            options,
            None,
        );
        let red = info.usage.iter().any(|(_, error)| *error);
        assert!(red, "{line}: {:?}", info.usage);
    }
}

#[test]
fn mine_count_is_a_whole_number_from_one() {
    use super::builtin::mine_count;
    assert_eq!(mine_count("40"), Some(40));
    assert_eq!(mine_count("1"), Some(1));
    for bad in ["0", "-3", "2.5", "many", ""] {
        assert_eq!(mine_count(bad), None, "{bad}");
    }
}

#[test]
fn follow_completes_tab_list_names_and_parses_bare() {
    let (t, _) = Table::with_scripts(Vec::new());
    let tree = super::tree::build(&t, Names::English);
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
    let tree = super::tree::build(&t, Names::English);
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

static ROWS: [(&str, &[&str], &str); 3] = [
    ("KillAura", &["ka"], "Убийца"),
    ("Reach", &[], "Дальность"),
    ("Speed", &[], "Reach"),
];

fn find_row(typed: &str) -> Option<&'static str> {
    pick(
        || ROWS.iter(),
        &norm(typed),
        |r| std::iter::once(r.0).chain(r.1.iter().copied()),
        |r| r.2,
    )
    .map(|r| r.0)
}

#[test]
fn names_match_in_english_aliases_and_as_drawn() {
    assert_eq!(find_row("killaura"), Some("KillAura"));
    assert_eq!(find_row("Kill Aura"), Some("KillAura"));
    assert_eq!(find_row("KA"), Some("KillAura"));
    assert_eq!(norm("Убийца Плюс"), "убийцаплюс");
    assert_eq!(find_row("УБИЙЦА"), Some("KillAura"));
    assert_eq!(find_row("дальность"), Some("Reach"));
    assert_eq!(find_row("reach"), Some("Reach"));
    assert_eq!(find_row("nothing"), None);
}

#[test]
fn the_russian_layout_rescue_reads_back_the_keys() {
    let (t, _) = Table::with_scripts(Vec::new());
    let command = |n: &str| t.find(n).map(|c| c.name());
    assert_eq!(remapped("ещппду", command), Some("toggle"));
    assert_eq!(remapped("ыуе", command), Some("set"));
    assert_eq!(
        remapped("tgogle", |n: &str| (n == "tgogle").then_some(())),
        None
    );
    assert_eq!(remapped("лшддфгкф", find_row), Some("KillAura"));
    assert_eq!(remapped("лшдд", find_row), None);
}

#[test]
fn a_tree_spells_each_row_once_or_twice() {
    assert_eq!(Names::English.spell("Kill Aura", "Убийца"), ["killaura"]);
    assert_eq!(Names::Drawn.spell("Kill Aura", "Убийца"), ["убийца"]);
    assert_eq!(
        Names::Both.spell("Kill Aura", "Убийца"),
        ["killaura", "убийца"]
    );
    assert_eq!(Names::Both.spell("ESP", "esp"), ["esp"]);
}

#[test]
fn the_popup_tree_follows_the_typed_script() {
    assert_eq!(for_line(".to"), Names::Both);
    assert_eq!(for_line(".toggle ki"), Names::Both);
    assert_eq!(for_line(".toggle уб"), Names::Both);
    assert_eq!(for_line(".toggle "), Names::Drawn);
    assert_eq!(for_line(".set killaura "), Names::English);
    assert_eq!(for_line(".set убийца "), Names::Drawn);
    assert_eq!(for_line(".set killaura дальность "), Names::Both);
    assert_eq!(for_line(".mine diamond 4 "), Names::English);
}

#[test]
fn the_both_tree_completes_english_input_as_the_english_tree() {
    let (t, _) = Table::with_scripts(Vec::new());
    let both = super::tree::build(&t, Names::Both);
    let options = Options {
        lead: '.',
        ..Options::CHAT
    };
    let line = ".toggle aim";
    let info = update_command_info(
        Some(&both),
        line,
        line.chars().count(),
        &Source::default(),
        options,
        None,
    );
    let names: Vec<String> = info.suggestions.list.into_iter().map(|s| s.text).collect();
    assert_eq!(names, complete(&t, line));
}
