mod candidates;
mod format;
mod parse;
mod suggestion;
mod tree;
mod usage;

#[cfg(test)]
pub(crate) mod tests;

pub use candidates::Source;
pub use suggestion::{Range, Suggestion, Suggestions};
pub use tree::{CommandTree, Node, NodeKind, Provider};

use crate::text::Span;

use candidates::{
    completion_suggestions, find_suggestion_context, local_suggestions, matches_sub_str,
};
use format::format_text;
use parse::{Parse, ParseError, error_context, exception_message, parse};
use usage::smart_usage;

#[derive(Default, Clone)]
pub struct CommandInfo {
    pub suggestions: Suggestions,
    pub ask_server: Option<String>,
    pub usage: Vec<(String, bool)>,
    pub usage_start: usize,
    pub spans: Vec<Span>,
    pub is_command: bool,
}

#[derive(Clone, Copy)]
pub struct Options {
    pub commands_only: bool,
    pub only_show_if_cursor_past_error: bool,
    pub lead: char,
}

impl Options {
    pub const CHAT: Options = Options {
        commands_only: false,
        only_show_if_cursor_past_error: false,
        lead: '/',
    };
    pub const COMMAND_ONLY: Options = Options {
        commands_only: true,
        only_show_if_cursor_past_error: true,
        lead: '/',
    };
}

pub fn update_command_info(
    tree: Option<&CommandTree>,
    value: &str,
    cursor: usize,
    source: &Source,
    options: Options,
    server_reply: Option<&Suggestions>,
) -> CommandInfo {
    let input: Vec<char> = value.chars().collect();
    let cursor = cursor.min(input.len());
    let mut info = CommandInfo::default();
    let starts_with_slash = input.first() == Some(&options.lead);
    let is_command = options.commands_only || starts_with_slash;
    let parse_start = usize::from(starts_with_slash);

    if !is_command {
        if value.trim().is_empty() {
            return info;
        }
        let start = last_word_index(&input[..cursor]);
        let remaining: String = input[start..cursor].iter().collect();
        let lower = remaining.to_lowercase();
        let values = source
            .custom_tab_suggestions()
            .into_iter()
            .filter(|v| matches_sub_str(&lower, &v.to_lowercase()));
        let items = local_suggestions(values, &remaining, Range::between(start, cursor)).collect();
        info.suggestions = Suggestions::create(&input, items);
        return info;
    }

    info.is_command = true;
    let Some(tree) = tree else { return info };

    let parse = parse(tree, &input, parse_start);
    info.spans = format_text(&parse, &input);
    let show_from = if options.only_show_if_cursor_past_error {
        parse.cursor
    } else {
        1
    };
    if cursor < show_from {
        return info;
    }
    let (parent, start_pos) = find_suggestion_context(&parse, cursor);
    let (suggestions, ask_server) =
        completion_suggestions(tree, parent, start_pos, &input, cursor, source);
    info.suggestions = suggestions;
    if ask_server {
        info.ask_server = Some(input[..cursor].iter().collect());
        let Some(reply) = server_reply else {
            return info;
        };
        info.suggestions = Suggestions::merge(
            &input,
            vec![std::mem::take(&mut info.suggestions), reply.clone()],
        );
    }

    let mut trailing_characters = false;
    if cursor == input.len() {
        if info.suggestions.is_empty() && !parse.errors.is_empty() {
            let mut literals = 0;
            for (_, error) in &parse.errors {
                if matches!(error, ParseError::LiteralIncorrect(_)) {
                    literals += 1;
                } else {
                    info.usage
                        .push((exception_message(error, &input, parse.cursor), true));
                }
            }
            if literals > 0 {
                info.usage.push((
                    format!(
                        "Incorrect argument for command at position {}: {}",
                        parse.cursor,
                        error_context(&input, parse.cursor)
                    ),
                    true,
                ));
            }
        } else if parse.cursor < input.len() {
            trailing_characters = true;
        }
    }

    if info.usage.is_empty() {
        let entries = smart_usage(tree, parent);
        if entries.is_empty() && trailing_characters {
            info.usage.push((parse_exception(&parse, &input), true));
        }
        info.usage.extend(entries.into_iter().map(|u| (u, false)));
    }
    info.usage_start = start_pos;
    info
}

fn parse_exception(parse: &Parse, input: &[char]) -> String {
    if parse.cursor >= input.len() {
        return String::new();
    }
    if parse.errors.len() == 1 {
        return exception_message(&parse.errors[0].1, input, parse.cursor);
    }
    let message = if parse.chain[0].range.is_empty() {
        "Unknown or incomplete command. See below for error"
    } else {
        "Incorrect argument for command"
    };
    format!(
        "{message} at position {}: {}",
        parse.cursor,
        error_context(input, parse.cursor)
    )
}

pub(crate) fn last_word_index(text: &[char]) -> usize {
    text.iter()
        .rposition(|c| c.is_whitespace())
        .map_or(0, |i| i + 1)
}

#[derive(Default)]
pub struct SuggestionClient {
    next_id: u32,
    inflight: Option<(u32, String)>,
    replies: Vec<(String, Suggestions)>,
}

const HELD_REPLIES: usize = 32;

impl SuggestionClient {
    pub fn request(&mut self, command: &str) -> Option<(u32, String)> {
        if self.inflight.as_ref().is_some_and(|(_, s)| s == command) {
            return None;
        }
        if self.reply_for(command).is_some() {
            return None;
        }
        self.next_id = self.next_id.wrapping_add(1);
        let id = self.next_id;
        self.inflight = Some((id, command.to_string()));
        Some((id, command.to_string()))
    }

    pub fn accept(&mut self, id: u32, suggestions: Suggestions) -> bool {
        match self.inflight.take() {
            Some((pending, command)) if pending == id => {
                self.replies.retain(|(s, _)| *s != command);
                if self.replies.len() == HELD_REPLIES {
                    self.replies.remove(0);
                }
                self.replies.push((command, suggestions));
                true
            }
            other => {
                self.inflight = other;
                false
            }
        }
    }

    pub fn reply_for(&self, command: &str) -> Option<&Suggestions> {
        self.replies
            .iter()
            .find(|(s, _)| s == command)
            .map(|(_, suggestions)| suggestions)
    }

    pub fn narrow(&self, command: &str) -> Option<Suggestions> {
        let mut held: Vec<&(String, Suggestions)> = self
            .replies
            .iter()
            .filter(|(asked, _)| command.starts_with(asked.as_str()))
            .collect();
        held.sort_by_key(|(asked, _)| std::cmp::Reverse(asked.len()));
        held.into_iter()
            .find_map(|(asked, reply)| narrow_reply(asked, reply, command))
    }
}

fn narrow_reply(asked: &str, reply: &Suggestions, command: &str) -> Option<Suggestions> {
    let tail = command.strip_prefix(asked)?;
    if reply.is_empty() || tail.chars().any(char::is_whitespace) {
        return None;
    }
    let input: Vec<char> = command.chars().collect();
    if reply.range.end != asked.chars().count() || reply.range.start > input.len() {
        return None;
    }
    let range = Range::between(reply.range.start, input.len());
    let typed: String = input[range.start..].iter().collect();
    let lower = typed.to_lowercase();
    let items: Vec<Suggestion> = reply
        .list
        .iter()
        .filter(|s| s.text != typed)
        .filter(|s| {
            let text = s.text.to_lowercase();
            let path = text.split_once(':').map_or(text.as_str(), |(_, p)| p);
            matches_sub_str(&lower, &text) || matches_sub_str(&lower, path)
        })
        .map(|s| Suggestion {
            range,
            text: s.text.clone(),
            tooltip: s.tooltip.clone(),
        })
        .collect();
    if items.is_empty() {
        return None;
    }
    Some(Suggestions::create(&input, items))
}
