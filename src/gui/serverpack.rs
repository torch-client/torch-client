use crate::gui::dialog::CONFIRM_BUTTON_WIDTH;
use crate::gui::painter::Painter;
use crate::gui::serverlist::PackPolicy;
use crate::gui::tooltip::translate;
use crate::gui::{GuiState, Screen, ScreenCtx, confirmlink};
use crate::session::SharedMutex;

pub(crate) fn draw(
    p: &mut Painter,
    ctx: &ScreenCtx,
    shared: &SharedMutex,
    state: &mut GuiState,
) -> Option<Screen> {
    let parent = state.server_pack_parent;
    let (prompt, address) = {
        let s = shared.lock().unwrap();
        (
            s.session.server_packs.prompt.clone(),
            s.session.current_address.clone(),
        )
    };
    let Some(prompt) = prompt else {
        return Some(parent);
    };
    let (title, line2, yes, no) = if prompt.required {
        (
            "multiplayer.requiredTexturePrompt.line1",
            "multiplayer.requiredTexturePrompt.line2",
            "gui.proceed",
            "menu.disconnect",
        )
    } else {
        (
            "multiplayer.texturePrompt.line1",
            "multiplayer.texturePrompt.line2",
            "gui.yes",
            "gui.no",
        )
    };
    let line2 = translate(line2, &[]);
    let message = match prompt.message {
        Some(text) => translate("multiplayer.texturePrompt.serverPrompt", &[line2, text]),
        None => line2,
    };
    let buttons = [translate(yes, &[]), translate(no, &[])];
    let chosen = confirmlink::draw_box(
        p,
        ctx,
        &translate(title, &[]),
        &message,
        None,
        &buttons,
        CONFIRM_BUTTON_WIDTH,
    );
    let accepted = match chosen {
        Some(i) => i == 0,
        None if ctx.input.escape => false,
        None => return None,
    };
    shared.lock().unwrap().session.server_packs.answer = Some(accepted);
    let remembered = match (accepted, prompt.required) {
        (true, _) => Some(PackPolicy::Enabled),
        (false, false) => Some(PackPolicy::Disabled),
        (false, true) => None,
    };
    if let (Some(policy), Some(address)) = (remembered, address) {
        state.menu.set_pack_policy(&address, policy);
    }
    Some(parent)
}
