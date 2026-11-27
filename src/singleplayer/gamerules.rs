#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Bool,
    Int { min: i32, max: i32 },
}

#[derive(Debug)]
pub(crate) struct Rule {
    pub id: &'static str,
    pub category: &'static str,
    pub kind: Kind,
    pub default: &'static str,
    pub description_id: &'static str,
}

const MAX: i32 = i32::MAX;

macro_rules! rule {
    ($id:literal, $category:literal, $kind:expr, $default:literal) => {
        Rule {
            id: $id,
            category: concat!("gamerule.category.minecraft.", $category),
            kind: $kind,
            default: $default,
            description_id: concat!("gamerule.minecraft.", $id),
        }
    };
}

#[rustfmt::skip]
pub(crate) const RULES: &[Rule] = &[
    rule!("advance_time", "updates", Kind::Bool, "true"),
    rule!("advance_weather", "updates", Kind::Bool, "true"),
    rule!("allow_entering_nether_using_portals", "misc", Kind::Bool, "true"),
    rule!("block_drops", "drops", Kind::Bool, "true"),
    rule!("block_explosion_drop_decay", "drops", Kind::Bool, "true"),
    rule!("command_blocks_work", "misc", Kind::Bool, "true"),
    rule!("command_block_output", "chat", Kind::Bool, "true"),
    rule!("drowning_damage", "player", Kind::Bool, "true"),
    rule!("elytra_movement_check", "player", Kind::Bool, "true"),
    rule!("ender_pearls_vanish_on_death", "player", Kind::Bool, "true"),
    rule!("entity_drops", "drops", Kind::Bool, "true"),
    rule!("fall_damage", "player", Kind::Bool, "true"),
    rule!("fire_damage", "player", Kind::Bool, "true"),
    rule!("fire_spread_radius_around_player", "updates", Kind::Int { min: -1, max: MAX }, "128"),
    rule!("forgive_dead_players", "mobs", Kind::Bool, "true"),
    rule!("freeze_damage", "player", Kind::Bool, "true"),
    rule!("global_sound_events", "misc", Kind::Bool, "true"),
    rule!("immediate_respawn", "player", Kind::Bool, "false"),
    rule!("keep_inventory", "player", Kind::Bool, "false"),
    rule!("lava_source_conversion", "updates", Kind::Bool, "false"),
    rule!("limited_crafting", "player", Kind::Bool, "false"),
    rule!("locator_bar", "player", Kind::Bool, "true"),
    rule!("log_admin_commands", "chat", Kind::Bool, "true"),
    rule!("max_block_modifications", "misc", Kind::Int { min: 1, max: MAX }, "32768"),
    rule!("max_command_forks", "misc", Kind::Int { min: 0, max: MAX }, "65536"),
    rule!("max_command_sequence_length", "misc", Kind::Int { min: 0, max: MAX }, "65536"),
    rule!("max_entity_cramming", "mobs", Kind::Int { min: 0, max: MAX }, "24"),
    rule!("max_snow_accumulation_height", "updates", Kind::Int { min: 0, max: 8 }, "1"),
    rule!("mob_drops", "drops", Kind::Bool, "true"),
    rule!("mob_explosion_drop_decay", "drops", Kind::Bool, "true"),
    rule!("mob_griefing", "mobs", Kind::Bool, "true"),
    rule!("natural_health_regeneration", "player", Kind::Bool, "true"),
    rule!("player_movement_check", "player", Kind::Bool, "true"),
    rule!("players_nether_portal_creative_delay", "player", Kind::Int { min: 0, max: MAX }, "0"),
    rule!("players_nether_portal_default_delay", "player", Kind::Int { min: 0, max: MAX }, "80"),
    rule!("players_sleeping_percentage", "player", Kind::Int { min: 0, max: MAX }, "100"),
    rule!("projectiles_can_break_blocks", "drops", Kind::Bool, "true"),
    rule!("pvp", "player", Kind::Bool, "true"),
    rule!("raids", "mobs", Kind::Bool, "true"),
    rule!("random_tick_speed", "updates", Kind::Int { min: 0, max: MAX }, "3"),
    rule!("reduced_debug_info", "misc", Kind::Bool, "false"),
    rule!("respawn_radius", "player", Kind::Int { min: 0, max: MAX }, "10"),
    rule!("send_command_feedback", "chat", Kind::Bool, "true"),
    rule!("show_advancement_messages", "chat", Kind::Bool, "true"),
    rule!("show_death_messages", "chat", Kind::Bool, "true"),
    rule!("spawner_blocks_work", "misc", Kind::Bool, "true"),
    rule!("spawn_mobs", "spawning", Kind::Bool, "true"),
    rule!("spawn_monsters", "spawning", Kind::Bool, "true"),
    rule!("spawn_patrols", "spawning", Kind::Bool, "true"),
    rule!("spawn_phantoms", "spawning", Kind::Bool, "true"),
    rule!("spawn_wandering_traders", "spawning", Kind::Bool, "true"),
    rule!("spawn_wardens", "spawning", Kind::Bool, "true"),
    rule!("spectators_generate_chunks", "player", Kind::Bool, "true"),
    rule!("spread_vines", "updates", Kind::Bool, "true"),
    rule!("tnt_explodes", "misc", Kind::Bool, "true"),
    rule!("tnt_explosion_drop_decay", "drops", Kind::Bool, "false"),
    rule!("universal_anger", "mobs", Kind::Bool, "false"),
    rule!("water_source_conversion", "updates", Kind::Bool, "true"),
];

pub(crate) fn parse(rule: &Rule, text: &str) -> Option<String> {
    match rule.kind {
        Kind::Bool => match read_string(text)?.as_str() {
            "true" => Some("true".to_owned()),
            "false" => Some("false".to_owned()),
            _ => None,
        },
        Kind::Int { min, max } => {
            if text.is_empty()
                || !text
                    .chars()
                    .all(|c| c.is_ascii_digit() || c == '.' || c == '-')
            {
                return None;
            }
            let n: i32 = text.parse().ok()?;
            (min..=max).contains(&n).then(|| n.to_string())
        }
    }
}

fn read_string(text: &str) -> Option<String> {
    let mut chars = text.chars();
    match chars.next()? {
        quote @ ('"' | '\'') => {
            let mut out = String::new();
            let mut escaped = false;
            while let Some(c) = chars.next() {
                if escaped {
                    if c != quote && c != '\\' {
                        return None;
                    }
                    out.push(c);
                    escaped = false;
                } else if c == '\\' {
                    escaped = true;
                } else if c == quote {
                    return chars.next().is_none().then_some(out);
                } else {
                    out.push(c);
                }
            }
            None
        }
        _ => {
            let unquoted =
                |c: char| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '+');
            text.chars().all(unquoted).then(|| text.to_owned())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_unique_and_defaults_parse() {
        let mut ids: Vec<&str> = RULES.iter().map(|r| r.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), RULES.len());
        for rule in RULES {
            assert_eq!(
                parse(rule, rule.default).as_deref(),
                Some(rule.default),
                "{}",
                rule.id
            );
        }
    }

    #[test]
    fn parse_matches_brigadier() {
        let int = RULES
            .iter()
            .find(|r| r.id == "max_snow_accumulation_height")
            .unwrap();
        assert_eq!(parse(int, "008").as_deref(), Some("8"));
        assert_eq!(parse(int, "9"), None);
        assert_eq!(parse(int, "+1"), None);
        assert_eq!(parse(int, " 1"), None);
        assert_eq!(parse(int, ""), None);
        let flag = RULES.iter().find(|r| r.id == "pvp").unwrap();
        assert_eq!(parse(flag, "\"false\"").as_deref(), Some("false"));
        assert_eq!(parse(flag, "True"), None);
        assert_eq!(parse(flag, "true "), None);
    }
}
