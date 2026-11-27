use crate::gui::painter::Painter;
use crate::gui::{ScreenCtx, confirmlink, tooltip};
use crate::play::commands::Check;
use crate::session::SharedMutex;
use crate::text::Span;

const BUTTON_WIDTH: f32 = 150.0;

#[derive(Clone, Debug, PartialEq)]
pub struct CommandPrompt {
    pub command: String,
    pub check: Check,
}

pub enum Answer {
    Accept,
    Back,
}

pub enum Accepted {
    Run(String),
    Chat(String),
    Copied,
}

pub fn check(command: &str, shared: &SharedMutex) -> Option<CommandPrompt> {
    let tree = {
        let s = shared.lock().unwrap();
        if s.session.reconfiguring || !s.in_world {
            return None;
        }
        s.session.command_tree.clone()
    };
    let check = match tree {
        Some(tree) => crate::play::commands::verify(&tree, command),
        None => Check::ParseErrors,
    };
    (check != Check::NoIssues).then(|| CommandPrompt {
        command: command.to_string(),
        check,
    })
}

impl CommandPrompt {
    pub fn draw(
        &self,
        p: &mut Painter,
        ctx: &ScreenCtx,
        had_screen: bool,
        after_is_none: bool,
    ) -> Option<Answer> {
        crate::gui::screens::dim_background(p, ctx.vw, ctx.vh);
        let accept = match self.check {
            Check::SignatureRequired if after_is_none => {
                "multiplayer.confirm_command.suggest_command"
            }
            Check::SignatureRequired => "chat.copy",
            _ => "multiplayer.confirm_command.run_command",
        };
        let buttons = [
            tooltip::translate(accept, &[]),
            tooltip::translate(if had_screen { "gui.back" } else { "gui.cancel" }, &[]),
        ];
        let chosen = confirmlink::draw_box_spans(
            p,
            ctx,
            &tooltip::translate("multiplayer.confirm_command.title", &[]),
            &self.message(),
            None,
            &buttons,
            BUTTON_WIDTH,
        )?;
        Some(if chosen == 0 {
            Answer::Accept
        } else {
            Answer::Back
        })
    }

    fn message(&self) -> Vec<Vec<Span>> {
        const HOLE: &str = "\u{0}";
        let key = self
            .check
            .message_key()
            .unwrap_or("multiplayer.confirm_command.parse_errors");
        let text = tooltip::translate(key, &[HOLE.to_string()]);
        text.split('\n')
            .map(|line| {
                let mut spans = Vec::new();
                for (i, part) in line.split(HOLE).enumerate() {
                    if i > 0 {
                        spans.push(Span::colored(self.command.clone(), crate::text::YELLOW));
                    }
                    if !part.is_empty() {
                        spans.push(Span::new(part));
                    }
                }
                spans
            })
            .collect()
    }

    pub fn accept(&self, after_is_none: bool) -> Accepted {
        match self.check {
            Check::SignatureRequired if after_is_none => {
                Accepted::Chat(format!("/{}", self.command))
            }
            Check::SignatureRequired => {
                crate::gui::render::clipboard_set(&format!("/{}", self.command));
                Accepted::Copied
            }
            _ => Accepted::Run(self.command.clone()),
        }
    }
}
