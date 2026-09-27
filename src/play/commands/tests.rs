use azalea_protocol::packets::game::c_commands::{
    BrigadierNodeStub, BrigadierNumber, BrigadierParser, BrigadierString, NodeType,
};

use super::candidates::matches_sub_str;
use super::format::{ARGUMENT_COLORS, LITERAL_COLOR, UNPARSED_COLOR, format_text};
use super::parse::{parse, parse_argument};
use super::usage::smart_usage;
use super::*;

fn literal(name: &str, children: Vec<u32>, executable: bool) -> BrigadierNodeStub {
    BrigadierNodeStub {
        is_executable: executable,
        children,
        redirect_node: None,
        node_type: NodeType::Literal {
            name: name.to_string(),
        },
        is_restricted: false,
    }
}

fn argument(
    name: &str,
    parser: BrigadierParser,
    children: Vec<u32>,
    executable: bool,
) -> BrigadierNodeStub {
    BrigadierNodeStub {
        is_executable: executable,
        children,
        redirect_node: None,
        node_type: NodeType::Argument {
            name: name.to_string(),
            parser,
            suggestions_type: None,
        },
        is_restricted: false,
    }
}

fn ask_server_argument(name: &str, children: Vec<u32>) -> BrigadierNodeStub {
    BrigadierNodeStub {
        is_executable: true,
        children,
        redirect_node: None,
        node_type: NodeType::Argument {
            name: name.to_string(),
            parser: BrigadierParser::String(BrigadierString::SingleWord),
            suggestions_type: Some(azalea_registry::identifier::Identifier::new(
                "minecraft:ask_server",
            )),
        },
        is_restricted: false,
    }
}

fn tree() -> CommandTree {
    let entity =
        BrigadierParser::Entity(azalea_protocol::packets::game::c_commands::EntityParser {
            single: false,
            players_only: false,
        });
    let mut stubs = vec![
        BrigadierNodeStub {
            is_executable: false,
            children: vec![1, 4, 7, 9, 13, 16, 17],
            redirect_node: None,
            node_type: NodeType::Root,
            is_restricted: false,
        },
        literal("gamemode", vec![2], false),
        argument("mode", BrigadierParser::GameMode, vec![3], true),
        argument("target", entity.clone(), vec![], true),
        literal("give", vec![5], false),
        argument("targets", entity.clone(), vec![6], false),
        argument("item", BrigadierParser::ItemStack, vec![], true),
        literal("teleport", vec![8], false),
        argument("location", BrigadierParser::BlockPos, vec![], true),
        literal("execute", vec![10, 11], false),
        literal("run", vec![], false),
        literal("as", vec![12], false),
        argument("targets", entity, vec![], false),
        literal("team", vec![14], false),
        literal("join", vec![15], false),
        ask_server_argument("team", vec![]),
        literal("gamerule", vec![], true),
        literal("setblock", vec![18], false),
        argument("block", BrigadierParser::BlockState, vec![], true),
    ];
    stubs[10].redirect_node = Some(0);
    stubs[12].redirect_node = Some(9);
    CommandTree::from_stubs(&stubs, 0)
}

fn chars(s: &str) -> Vec<char> {
    s.chars().collect()
}

fn texts(s: &Suggestions) -> Vec<String> {
    s.list.iter().map(|x| x.text.clone()).collect()
}

fn complete(value: &str) -> CommandInfo {
    let cursor = value.chars().count();
    update_command_info(
        Some(&tree()),
        value,
        cursor,
        &Source::default(),
        Options::CHAT,
    )
}

#[test]
fn root_literals_filter_by_prefix() {
    assert_eq!(
        texts(&complete("/g").suggestions),
        vec![
            "gamemode".to_string(),
            "gamerule".to_string(),
            "give".to_string()
        ]
    );
    assert_eq!(
        texts(&complete("/gamem").suggestions),
        vec!["gamemode".to_string()]
    );
    assert_eq!(complete("/").suggestions.list.len(), 7);
}

#[test]
fn suggestion_replaces_only_its_word() {
    let info = complete("/gamem");
    let s = &info.suggestions.list[0];
    assert_eq!(s.range, Range::between(1, 6));
    assert_eq!(s.apply(&chars("/gamem")), "/gamemode");
}

#[test]
fn trailing_space_suggests_the_next_node() {
    let info = complete("/gamemode ");
    assert_eq!(
        texts(&info.suggestions),
        vec![
            "adventure".to_string(),
            "creative".to_string(),
            "spectator".to_string(),
            "survival".to_string()
        ]
    );
    assert_eq!(info.suggestions.range, Range::between(10, 10));
    assert_eq!(
        info.suggestions.list[0].apply(&chars("/gamemode ")),
        "/gamemode adventure"
    );
}

#[test]
fn cursor_mid_word_completes_that_word() {
    let value = "/gamemode cre extra";
    let info = update_command_info(Some(&tree()), value, 13, &Source::default(), Options::CHAT);
    assert_eq!(texts(&info.suggestions), vec!["creative".to_string()]);
    let applied = info.suggestions.list[0].apply(&chars(value));
    assert_eq!(applied, "/gamemode creative extra");
}

#[test]
fn nested_literals_resolve_through_their_parent() {
    assert_eq!(
        texts(&complete("/team ").suggestions),
        vec!["join".to_string()]
    );
    assert!(complete("/nosuch ").suggestions.is_empty());
}

#[test]
fn ask_server_nodes_produce_a_request() {
    let info = complete("/team join re");
    assert!(info.suggestions.is_empty());
    assert_eq!(info.ask_server.as_deref(), Some("/team join re"));
    assert_eq!(complete("/gamemode ").ask_server, None);
}

#[test]
fn redirect_restarts_at_the_target() {
    let info = complete("/execute run ");
    assert_eq!(info.suggestions.list.len(), 7);
    assert_eq!(info.suggestions.range, Range::between(13, 13));
    assert_eq!(
        texts(&complete("/execute run gamer").suggestions),
        vec!["gamerule".to_string()]
    );
}

#[test]
fn redirect_to_an_ancestor_loops() {
    let info = complete("/execute as Steve ");
    let mut got = texts(&info.suggestions);
    got.sort();
    assert_eq!(got, vec!["as".to_string(), "run".to_string()]);
}

#[test]
fn executable_marks_following_arguments_optional() {
    let t = tree();
    assert_eq!(smart_usage(&t, 1), vec!["<mode> [<target>]".to_string()]);
    assert_eq!(smart_usage(&t, 2), vec!["[<target>]".to_string()]);
    assert!(smart_usage(&t, 0).is_empty());
}

#[test]
fn unparsed_tail_is_red() {
    let input = chars("/gamemode nonsense");
    let p = parse(&tree(), &input, 1);
    let spans = format_text(&p, &input);
    let red: String = spans
        .iter()
        .filter(|s| s.style.color == UNPARSED_COLOR)
        .map(|s| s.text.as_str())
        .collect();
    assert_eq!(red, "");

    let input = chars("/teleport ");
    let p = parse(&tree(), &input, 1);
    assert_eq!(p.cursor, 9);
}

#[test]
fn arguments_take_the_colour_cycle() {
    let input = chars("/gamemode creative Steve");
    let p = parse(&tree(), &input, 1);
    let spans = format_text(&p, &input);
    let colors: Vec<(String, u32)> = spans
        .iter()
        .map(|s| (s.text.clone(), s.style.color))
        .collect();
    assert_eq!(
        colors,
        vec![
            ("/gamemode ".to_string(), LITERAL_COLOR),
            ("creative".to_string(), ARGUMENT_COLORS[0]),
            (" ".to_string(), LITERAL_COLOR),
            ("Steve".to_string(), ARGUMENT_COLORS[1]),
        ]
    );
}

#[test]
fn coordinates_complete_the_remaining_axes() {
    assert_eq!(
        texts(&complete("/teleport ").suggestions),
        vec!["~".to_string(), "~ ~".to_string(), "~ ~ ~".to_string()]
    );
    assert_eq!(
        texts(&complete("/teleport 10").suggestions),
        vec!["10 ~".to_string(), "10 ~ ~".to_string()]
    );
}

#[test]
fn a_word_that_is_not_a_coordinate_offers_nothing() {
    assert!(complete("/teleport Steve").suggestions.is_empty());
    assert!(complete("/teleport 1 nope").suggestions.is_empty());
}

#[test]
fn local_coordinates_complete_with_carets() {
    assert_eq!(
        texts(&complete("/teleport ^").suggestions),
        vec!["^ ^".to_string(), "^ ^ ^".to_string()]
    );
    assert_eq!(
        texts(&complete("/teleport ^1 ^").suggestions),
        vec!["^1 ^ ^".to_string()]
    );
    assert!(complete("/teleport ~ ^").suggestions.is_empty());
}

#[test]
fn teleport_completes_its_second_argument() {
    let source = Source {
        player_names: vec!["Steve".to_string()],
        custom_completions: Vec::new(),
    };
    let complete = |value: &str| {
        update_command_info(
            Some(&teleport_tree()),
            value,
            value.chars().count(),
            &source,
            Options::CHAT,
        )
    };
    let input = chars("/tp 1 2 3");
    let p = parse(&teleport_tree(), &input, 1);
    assert_eq!(p.cursor, input.len());
    assert_eq!(p.last().nodes.len(), 1, "one <location>, not three names");

    assert_eq!(
        texts(&complete("/tp Steve ").suggestions),
        vec![
            "@a".to_string(),
            "@e".to_string(),
            "@n".to_string(),
            "@p".to_string(),
            "@r".to_string(),
            "@s".to_string(),
            "Steve".to_string(),
            "~".to_string(),
            "~ ~".to_string(),
            "~ ~ ~".to_string(),
        ]
    );
    assert_eq!(
        texts(&complete("/tp Steve 10").suggestions),
        vec!["10 ~".to_string(), "10 ~ ~".to_string()]
    );
    assert_eq!(
        texts(&complete("/tp Steve St").suggestions),
        vec!["Steve".to_string()]
    );
}

#[test]
fn selectors_complete_from_the_at_sign() {
    let info = update_command_info(
        Some(&teleport_tree()),
        "/tp @",
        5,
        &Source::default(),
        Options::CHAT,
    );
    assert_eq!(
        texts(&info.suggestions),
        vec![
            "@a".to_string(),
            "@e".to_string(),
            "@n".to_string(),
            "@p".to_string(),
            "@r".to_string(),
            "@s".to_string(),
        ]
    );
    let info = update_command_info(
        Some(&teleport_tree()),
        "/tp @e[type=",
        12,
        &Source::default(),
        Options::CHAT,
    );
    assert_eq!(info.ask_server.as_deref(), Some("/tp @e[type="));
}

fn teleport_tree() -> CommandTree {
    let entities =
        BrigadierParser::Entity(azalea_protocol::packets::game::c_commands::EntityParser {
            single: false,
            players_only: false,
        });
    let entity =
        BrigadierParser::Entity(azalea_protocol::packets::game::c_commands::EntityParser {
            single: true,
            players_only: false,
        });
    let mut stubs = vec![
        BrigadierNodeStub {
            is_executable: false,
            children: vec![1, 6],
            redirect_node: None,
            node_type: NodeType::Root,
            is_restricted: false,
        },
        literal("teleport", vec![2, 4, 5], false),
        argument("targets", entities, vec![3, 7], false),
        argument("location", BrigadierParser::Vec3, vec![], true),
        argument("location", BrigadierParser::Vec3, vec![], true),
        argument("destination", entity.clone(), vec![], true),
        literal("tp", vec![], false),
        argument("destination", entity, vec![], true),
    ];
    stubs[6].redirect_node = Some(1);
    CommandTree::from_stubs(&stubs, 0)
}

#[test]
fn block_ids_come_from_the_block_registry() {
    assert_eq!(
        texts(&complete("/setblock end_portal").suggestions),
        vec![
            "minecraft:end_portal".to_string(),
            "minecraft:end_portal_frame".to_string()
        ]
    );
    assert_eq!(
        texts(&complete("/setblock nether_por").suggestions),
        vec!["minecraft:nether_portal".to_string()]
    );
    assert!(
        !texts(&complete("/setblock diamond_s").suggestions)
            .contains(&"minecraft:diamond_sword".to_string())
    );
}

#[test]
fn item_ids_match_on_the_path() {
    let info = complete("/give Steve diamond_sw");
    assert_eq!(
        texts(&info.suggestions),
        vec!["minecraft:diamond_sword".to_string()]
    );
    let info = complete("/give Steve minecraft:diamond_sw");
    assert_eq!(
        texts(&info.suggestions),
        vec!["minecraft:diamond_sword".to_string()]
    );
}

#[test]
fn sub_str_matching_follows_the_splitters() {
    assert!(matches_sub_str("shulker", "blue_shulker_box"));
    assert!(matches_sub_str("box", "blue_shulker_box"));
    assert!(matches_sub_str("", "anything"));
    assert!(!matches_sub_str("hulker", "blue_shulker_box"));
    assert!(!matches_sub_str("stone", "minecraft:stone"));
}

#[test]
fn plain_chat_completes_player_names() {
    let source = Source {
        player_names: vec!["Alex".to_string(), "Steve".to_string()],
        custom_completions: vec!["!help".to_string()],
    };
    let info = update_command_info(None, "hey St", 6, &source, Options::CHAT);
    assert!(!info.is_command);
    assert_eq!(texts(&info.suggestions), vec!["Steve".to_string()]);
    assert_eq!(info.suggestions.range, Range::between(4, 6));
    assert!(
        update_command_info(None, "   ", 3, &source, Options::CHAT)
            .suggestions
            .is_empty()
    );
}

#[test]
fn requests_are_not_repeated() {
    let mut client = SuggestionClient::default();
    let first = client.request("/team join re").unwrap();
    assert_eq!(first.0, 1);
    assert!(client.request("/team join re").is_none());
    let second = client.request("/team join red").unwrap();
    assert_eq!(second.0, 2);

    client.accept(2, Suggestions::default());
    assert!(client.request("/team join red").is_none());
    assert!(client.request("/team join re").is_some());
}

#[test]
fn stale_replies_are_discarded() {
    let mut client = SuggestionClient::default();
    client.request("/team join r");
    client.request("/team join re");
    let answer = Suggestions::create(
        &chars("/team join re"),
        vec![Suggestion {
            range: Range::between(11, 13),
            text: "red".to_string(),
            tooltip: None,
        }],
    );
    assert!(
        !client.accept(1, answer.clone()),
        "the id 1 request was superseded"
    );
    assert!(client.reply_for("/team join re").is_none());
    assert!(client.accept(2, answer));
    assert_eq!(
        client.reply_for("/team join re").map(texts),
        Some(vec!["red".to_string()])
    );
}

#[test]
fn merging_widens_every_entry_to_one_range() {
    let command = chars("/team join re");
    let local = Suggestions::create(
        &command,
        vec![Suggestion {
            range: Range::between(11, 13),
            text: "reds".to_string(),
            tooltip: None,
        }],
    );
    let remote = Suggestions::create(
        &command,
        vec![Suggestion {
            range: Range::between(6, 13),
            text: "join red".to_string(),
            tooltip: None,
        }],
    );
    let merged = Suggestions::merge(&command, vec![local, remote]);
    assert_eq!(merged.range, Range::between(6, 13));
    assert_eq!(
        texts(&merged),
        vec!["join red".to_string(), "join reds".to_string()]
    );
}

#[test]
fn unknown_command_reports_an_error() {
    let info = complete("/nosuch");
    assert_eq!(info.usage.len(), 1);
    assert!(
        info.usage[0]
            .0
            .starts_with("Unknown or incomplete command. See below for error"),
        "{:?}",
        info.usage[0]
    );
    assert!(info.usage[0].1, "an error line, not a usage line");
    assert!(info.usage[0].0.ends_with("<--[HERE]"));

    let info = complete("/gamemode creative Steve trailing");
    assert!(!info.usage.is_empty());
    assert!(info.usage[0].1, "{:?}", info.usage);
}

#[test]
fn usage_lines_come_from_the_node_under_the_cursor() {
    let info = complete("/gamemode ");
    assert_eq!(info.usage, vec![("<mode> [<target>]".to_string(), false)]);
    assert_eq!(info.usage_start, 10);
}

#[test]
fn quoted_arguments_parse_as_one_token() {
    let input = chars("\"two words\" tail");
    let mut cursor = 0;
    parse_argument(
        &BrigadierParser::String(BrigadierString::QuotablePhrase),
        &input,
        &mut cursor,
    )
    .unwrap();
    assert_eq!(cursor, 11);
}

#[test]
fn greedy_phrases_take_the_rest() {
    let input = chars("all the rest");
    let mut cursor = 0;
    parse_argument(&BrigadierParser::Message, &input, &mut cursor).unwrap();
    assert_eq!(cursor, input.len());
}

#[test]
fn numeric_bounds_are_enforced() {
    let parser = BrigadierParser::Integer(BrigadierNumber::new(Some(0), Some(10)));
    let mut cursor = 0;
    assert!(parse_argument(&parser, &chars("5"), &mut cursor).is_ok());
    let mut cursor = 0;
    let err = parse_argument(&parser, &chars("50"), &mut cursor).unwrap_err();
    assert_eq!(err, "Integer must not be more than 10: found 50");
}
