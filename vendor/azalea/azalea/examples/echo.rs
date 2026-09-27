use azalea::prelude::*;

#[tokio::main]
async fn main() -> AppExit {
    let account = Account::offline("bot");

    ClientBuilder::new()
        .set_handler(handle)
        .start(account, "localhost")
        .await
}

#[derive(Clone, Component, Default)]
pub struct State {}

async fn handle(bot: Client, event: Event, _state: State) -> eyre::Result<()> {
    if let Event::Chat(m) = event
        && let (Some(sender), content) = m.split_sender_and_content()
    {
        if sender == bot.username() {
            return Ok(());
        }
        bot.chat(content);
    }

    Ok(())
}
