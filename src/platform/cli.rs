#[derive(Default)]
pub(crate) struct Args {
    pub username: Option<String>,
    pub address: Option<String>,
    #[cfg_attr(
        any(target_arch = "wasm32", not(feature = "online_mode")),
        allow(
            dead_code,
            reason = "no command line on the web, and nothing to authenticate with offline"
        )
    )]
    pub access_token: Option<String>,
}

pub(crate) const DEFAULT_USERNAME: &str = "bot";

pub(crate) fn valid_username(name: &str) -> bool {
    (3..=16).contains(&name.len()) && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Action {
    Dir,
    Reset { scope: Scope, assume_yes: bool },
    Env,
    Ping(String),
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Scope {
    All,
    Config,
    Assets,
}

#[cfg(not(target_arch = "wasm32"))]
impl Scope {
    fn flag(self) -> &'static str {
        match self {
            Scope::All => "--reset",
            Scope::Config => "--reset-config",
            Scope::Assets => "--reset-assets",
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) enum Command {
    Play(Args),
    Run(Action),
}

#[cfg(not(target_arch = "wasm32"))]
const HELP: &str = "\
A from-scratch Minecraft client.

USAGE
    torch-client [OPTIONS] [ADDRESS]
    torch-client [OPTIONS] <USERNAME> <ADDRESS>
    torch-client <ACTION>

ARGUMENTS
    One argument is the address. Two are the username and then the address.
    With no address the client opens on the title screen and the multiplayer
    menu decides what to connect to.

OPTIONS
    -u, --username <NAME>   Name to join under. Three to sixteen characters,
                            letters, digits and underscore. Without this the
                            name of the active account is used, and with no
                            account the client opens on the screen that makes
                            one.
    -a, --address <ADDR>    Server to connect to. Same as the positional form.
        --ip <ADDR>         Alias for --address.
    -h, --help              Print this and exit.
    -V, --version           Print the version and exit.

ACCOUNTS
    A username on its own is an offline join: the name is whatever was typed
    and no server checks it, which is what a cracked server accepts.
";

#[cfg(all(feature = "online_mode", not(target_arch = "wasm32")))]
const HELP_TOKEN: &str = "\
    -t, --access-token <T>  Join online-mode servers as the account this token
                            belongs to. It is a Minecraft session token (the
                            `access_token` of a `login_with_xbox` reply), not a
                            Microsoft OAuth token. The client asks Mojang whose
                            it is before opening a window and exits if the
                            answer is not an account, so the name and uuid come
                            from the profile rather than from --username; the
                            two together are an error when they disagree.
                            Nothing is written to disk: the token lives for the
                            one run. It is visible in the shell history and in
                            `ps`, so prefer signing in through the account
                            screen for anything but a scripted run.
";

#[cfg(all(not(feature = "online_mode"), not(target_arch = "wasm32")))]
const HELP_TOKEN: &str = "\
    --access-token needs the `online_mode` feature, which this build does not
    have. Rebuild with `cargo build --release --features online_mode`.
";

#[cfg(not(target_arch = "wasm32"))]
const HELP_ACTIONS: &str = "
ACTIONS
    Each runs on its own and exits; none of them starts the game.

        --dir               Print the working directory, what is in it, and
                            which environment variable moves each part.
        --reset             Delete the working directory outright: options,
                            keybinds, the server list, the accounts, and the
                            downloaded assets. Asks first unless --yes is
                            given, and refuses to run unattended without it.
        --reset-config      Delete only the documents this client writes
                            (options, keybinds, servers, accounts) and leave
                            the assets alone.
        --reset-assets      Delete only the downloaded asset store. The next
                            run downloads it again.
        --env               Print the environment variables this build reads,
                            what each does and what it does when unset.
        --ping <ADDRESS>    Ask one server for its status and print the MOTD,
                            the player count, the sample of names it advertises,
                            its version and protocol, and the round trip. Exits
                            non-zero if it does not answer, so a script can use
                            it to ask whether a server is up. Colours the MOTD
                            when it is printing to a terminal and not when it is
                            printing to a pipe.
    -y, --yes               Answer yes to a reset's confirmation.

    A reset never touches an asset tree named by MINECRAFT_ASSETS: that is a
    directory this client reads and does not own.
";

#[cfg(not(target_arch = "wasm32"))]
const HELP_ADDRESSES: &str = "
ADDRESSES
    host, or host:port      A Minecraft server over TCP. Port defaults to 25565.
";

#[cfg(all(feature = "eagler", not(target_arch = "wasm32")))]
const HELP_EAGLER: &str = "    wss://host[/path]       An Eaglercraft server over WebSocket, via
                            EaglerXServer. Port defaults to 443, or 80 for
                            ws://. The path is part of the URL and is kept, so
                            proxies that route by path work.
";

#[cfg(not(any(feature = "eagler", target_arch = "wasm32")))]
const HELP_EAGLER: &str =
    "    wss:// addresses need the `eagler` feature, which this build does not have.
    Rebuild with `cargo build --release --features eagler`.
";

#[cfg(not(target_arch = "wasm32"))]
const EXAMPLES: &str = "
EXAMPLES
    torch-client
    torch-client localhost:25565
    torch-client Steve play.example.com
    torch-client --username Steve --address play.example.com
    torch-client -u Steve --ip wss://play.example.net
    torch-client --access-token \"$TOKEN\" play.example.com
    torch-client --dir
    torch-client --reset-config
    torch-client --reset --yes
    torch-client --env
    torch-client --ping play.example.com
";

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn parse() -> Args {
    match parse_from(std::env::args().skip(1)) {
        Ok(Command::Play(args)) => args,
        Ok(Command::Run(action)) => std::process::exit(run_action(action)),
        Err(message) => {
            eprintln!("{message}\n\nTry `torch-client --help`.");
            std::process::exit(2);
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn parse_from(args: impl Iterator<Item = String>) -> Result<Command, String> {
    let mut username = None;
    let mut address = None;
    let mut access_token = None;
    let mut action: Option<Action> = None;
    let mut assume_yes = false;
    let mut positional = Vec::new();
    let mut args = args;

    let set_action = |action: &mut Option<Action>, wanted: Action| -> Result<(), String> {
        match action {
            Some(first) if *first != wanted => Err(format!(
                "`{}` and `{}` are two different jobs; run one at a time.",
                name_of(first),
                name_of(&wanted)
            )),
            _ => {
                *action = Some(wanted);
                Ok(())
            }
        }
    };

    while let Some(arg) = args.next() {
        let mut value = |name: &str| -> Result<String, String> {
            args.next().ok_or_else(|| format!("{name} needs a value."))
        };
        match arg.as_str() {
            "-h" | "--help" => {
                print!("{HELP}{HELP_TOKEN}{HELP_ACTIONS}{HELP_ADDRESSES}{HELP_EAGLER}{EXAMPLES}");
                std::process::exit(0);
            }
            "-V" | "--version" => {
                println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }
            "-u" | "--username" => username = Some(value("--username")?),
            "-a" | "--address" | "--ip" | "--server" => address = Some(value("--address")?),
            "-t" | "--access-token" => access_token = Some(value("--access-token")?),
            "--dir" | "--print-dir" => set_action(&mut action, Action::Dir)?,
            "--reset" | "--reset-all" => set_action(&mut action, reset(Scope::All))?,
            "--reset-config" => set_action(&mut action, reset(Scope::Config))?,
            "--reset-assets" => set_action(&mut action, reset(Scope::Assets))?,
            "--env" | "--print-env" => set_action(&mut action, Action::Env)?,
            "--ping" => set_action(&mut action, Action::Ping(value("--ping")?))?,
            "-y" | "--yes" => assume_yes = true,
            other if other.starts_with('-') && other != "-" => {
                return Err(format!("Unknown option `{other}`."));
            }
            other => positional.push(other.to_string()),
        }
    }

    match positional.len() {
        0 => {}
        1 => address = address.or_else(|| Some(positional.remove(0))),
        2 => {
            let addr = positional.pop().expect("two positionals");
            let user = positional.pop().expect("two positionals");
            username = username.or(Some(user));
            address = address.or(Some(addr));
        }
        n => return Err(format!("Expected at most two arguments, got {n}.")),
    }

    if let Some(action) = &mut action {
        if username.is_some() || address.is_some() || access_token.is_some() {
            return Err(format!("`{}` takes no other arguments.", name_of(action)));
        }
        if let Action::Reset {
            assume_yes: yes, ..
        } = action
        {
            *yes = assume_yes;
        } else if assume_yes {
            return Err(format!(
                "`--yes` answers a reset's question; `{}` does not ask one.",
                name_of(action)
            ));
        }
        if let Action::Ping(target) = action {
            crate::platform::address::check(target)?;
        }
    } else if assume_yes {
        return Err("`--yes` only means something with a reset.".to_string());
    }

    if let Some(username) = &username
        && !valid_username(username)
    {
        return Err(format!(
            "`{username}` is not a usable name. Three to sixteen characters, letters, digits and \
             underscore only."
        ));
    }

    if let Some(token) = &access_token {
        if token.trim().is_empty() {
            return Err("--access-token needs a token.".to_string());
        }
        if !cfg!(feature = "online_mode") {
            return Err(
                "--access-token needs the `online_mode` feature, which this build does not have. \
                 Rebuild with `cargo build --release --features online_mode`."
                    .to_string(),
            );
        }
    }

    let address = address
        .map(|a| a.trim().to_string())
        .filter(|a| !a.is_empty());
    if let Some(address) = &address {
        crate::platform::address::check(address)?;
    }

    if let Some(action) = action {
        return Ok(Command::Run(action));
    }
    Ok(Command::Play(Args {
        username,
        address,
        access_token,
    }))
}

#[cfg(not(target_arch = "wasm32"))]
fn reset(scope: Scope) -> Action {
    Action::Reset {
        scope,
        assume_yes: false,
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn name_of(action: &Action) -> &'static str {
    match action {
        Action::Dir => "--dir",
        Action::Reset { scope, .. } => scope.flag(),
        Action::Env => "--env",
        Action::Ping(_) => "--ping",
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn run_action(action: Action) -> i32 {
    match action {
        Action::Dir => {
            print_dirs();
            0
        }
        Action::Env => {
            print_env();
            0
        }
        Action::Reset { scope, assume_yes } => delete(scope, assume_yes),
        Action::Ping(address) => ping(&address),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn print_env() {
    let table: Vec<_> = crate::platform::env::table().collect();
    let width = table.iter().map(|v| v.name.len()).max().unwrap_or(0);
    println!("Environment variables this build reads:\n");
    for var in table {
        println!("  {:width$}  {}", var.name, var.what);
        println!("  {:width$}  unset: {}", "", var.default);
    }
    println!(
        "\nOnly what this build acts on is listed; other features add more.\nSet in the usual way: \
         `MC_DEBUG=net,chunks torch-client play.example.com`."
    );
}

#[cfg(not(target_arch = "wasm32"))]
fn print_dirs() {
    println!("Working directory");
    let config = crate::platform::storage::dir();
    row("config", &config, "MC_CLIENT_CONFIG_DIR");
    if let Some(assets) = asset_store() {
        row("assets", &assets, "MC_ASSET_DIR");
    }
    println!("\nDocuments");
    for slot in crate::platform::storage::ALL {
        let path = slot.path();
        let state = match std::fs::metadata(&path) {
            Ok(meta) => human_size(meta.len()),
            Err(_) => "-".to_string(),
        };
        println!("  {:16}  {:>10}", slot.0, state);
    }
    println!("\nAssets are read from");
    println!("  {}", crate::assets_root().display());
    if cfg!(feature = "asset_download") && std::env::var_os("MINECRAFT_ASSETS").is_none() {
        println!("  (a downloaded set, addressed by that path rather than stored at it)");
    }
    println!("  MINECRAFT_ASSETS moves it, and no reset ever deletes it");
}

#[cfg(not(target_arch = "wasm32"))]
fn row(label: &str, path: &std::path::Path, var: &str) {
    let state = match walk(path) {
        Some((files, bytes)) => format!("{files} files, {}", human_size(bytes)),
        None => "missing".to_string(),
    };
    println!("  {label:8}  {}", path.display());
    println!("  {:8}  {state}, moved by {var}", "");
}

#[cfg(not(target_arch = "wasm32"))]
fn asset_store() -> Option<std::path::PathBuf> {
    #[cfg(feature = "asset_download")]
    {
        Some(crate::client::assets::root())
    }
    #[cfg(not(feature = "asset_download"))]
    {
        None
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn walk(dir: &std::path::Path) -> Option<(u64, u64)> {
    let entries = std::fs::read_dir(dir).ok()?;
    let mut files = 0;
    let mut bytes = 0;
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            if let Some((f, b)) = walk(&entry.path()) {
                files += f;
                bytes += b;
            }
        } else {
            files += 1;
            bytes += entry.metadata().map(|m| m.len()).unwrap_or(0);
        }
    }
    Some((files, bytes))
}

#[cfg(not(target_arch = "wasm32"))]
fn human_size(bytes: u64) -> String {
    match bytes {
        0..1_000 => format!("{bytes} B"),
        1_000..1_000_000 => format!("{:.1} kB", bytes as f64 / 1e3),
        1_000_000..1_000_000_000 => format!("{:.1} MB", bytes as f64 / 1e6),
        _ => format!("{:.1} GB", bytes as f64 / 1e9),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn delete(scope: Scope, assume_yes: bool) -> i32 {
    let config = crate::platform::storage::dir();
    let mut dirs: Vec<std::path::PathBuf> = Vec::new();
    let mut files: Vec<std::path::PathBuf> = Vec::new();
    match scope {
        Scope::Config => files.extend(
            crate::platform::storage::ALL
                .iter()
                .map(|slot| slot.path())
                .filter(|path| path.exists()),
        ),
        Scope::Assets => match asset_store() {
            Some(store) => dirs.push(store),
            None => {
                eprintln!(
                    "This build has no downloaded assets: --reset-assets needs the \
                     `asset_download` feature. Nothing was deleted."
                );
                return 2;
            }
        },
        Scope::All => {
            dirs.push(config.clone());
            if let Some(store) = asset_store()
                && !store.starts_with(&config)
            {
                dirs.push(store);
            }
        }
    }
    dirs.retain(|dir| dir.exists());

    if dirs.is_empty() && files.is_empty() {
        println!("{}: nothing to delete.", scope.flag());
        return 0;
    }

    for dir in &dirs {
        if let Err(message) = removable(dir) {
            eprintln!("{message} Nothing was deleted.");
            return 2;
        }
    }

    println!("This deletes:");
    for dir in &dirs {
        let state = walk(dir)
            .map(|(count, bytes)| format!("{count} files, {}", human_size(bytes)))
            .unwrap_or_else(|| "empty".to_string());
        println!("  {} ({state})", dir.display());
    }
    for file in &files {
        println!("  {}", file.display());
    }

    match confirm(assume_yes) {
        Err(message) => {
            eprintln!("{message}");
            return 2;
        }
        Ok(false) => {
            println!("Nothing was deleted.");
            return 0;
        }
        Ok(true) => {}
    }

    let mut failed = false;
    for dir in &dirs {
        if let Err(e) = std::fs::remove_dir_all(dir) {
            eprintln!("Could not delete {}: {e}", dir.display());
            failed = true;
        }
    }
    for file in &files {
        if let Err(e) = std::fs::remove_file(file) {
            eprintln!("Could not delete {}: {e}", file.display());
            failed = true;
        }
    }
    if failed {
        return 1;
    }
    println!("Done. The next run starts from nothing.");
    0
}

#[cfg(not(target_arch = "wasm32"))]
fn ping(address: &str) -> i32 {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(e) => {
            eprintln!("Could not start a runtime to ping with: {e}");
            return 1;
        }
    };
    let status = match runtime.block_on(crate::gui::ping::status_once(address, false)) {
        Ok(status) => status,
        Err(message) => {
            eprintln!("{address}: {message}");
            return 1;
        }
    };

    let colour = std::io::IsTerminal::is_terminal(&std::io::stdout());
    println!("{address}");
    for (i, line) in motd(&status.motd, colour).iter().enumerate() {
        let label = if i == 0 { "MOTD" } else { "" };
        println!("  {label:<9} {line}");
    }
    println!("  {:<9} {}/{}", "Players", status.online, status.max);
    let names: Vec<(String, String)> = status
        .sample
        .iter()
        .map(|name| {
            let spans = crate::text::parse_formatted(name);
            let mut line = pieces(&spans).swap_remove(0);
            trim(&mut line);
            (plain(&line), draw(&line, colour))
        })
        .filter(|(text, _)| !text.is_empty())
        .collect();
    for (i, line) in wrap(&names, 58).iter().enumerate() {
        let label = if i == 0 { "Sample" } else { "" };
        println!("  {label:<9} {line}");
    }
    let ours = azalea_protocol::packets::PROTOCOL_VERSION;
    let note = if status.protocol == ours {
        String::new()
    } else {
        format!(", this client speaks {ours}")
    };
    println!(
        "  {:<9} {} (protocol {}{note})",
        "Version", status.version, status.protocol
    );
    println!("  {:<9} {} ms", "Latency", status.latency_ms);
    0
}

#[cfg(not(target_arch = "wasm32"))]
type Piece<'a> = (&'a str, &'a crate::text::Style);

#[cfg(not(target_arch = "wasm32"))]
fn pieces(spans: &[crate::text::Span]) -> Vec<Vec<Piece<'_>>> {
    let mut lines = vec![Vec::new()];
    for span in spans {
        for (i, text) in span.text.split('\n').enumerate() {
            if i > 0 {
                lines.push(Vec::new());
            }
            lines
                .last_mut()
                .expect("a line to append to")
                .push((text, &span.style));
        }
    }
    lines
}

#[cfg(not(target_arch = "wasm32"))]
fn trim(line: &mut Vec<Piece<'_>>) {
    while line.len() > 1 && line[0].0.trim_start().is_empty() {
        line.remove(0);
    }
    if let Some(first) = line.first_mut() {
        first.0 = first.0.trim_start();
    }
    while line.len() > 1 && line[line.len() - 1].0.trim_end().is_empty() {
        line.pop();
    }
    if let Some(last) = line.last_mut() {
        last.0 = last.0.trim_end();
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn plain(line: &[Piece<'_>]) -> String {
    line.iter().map(|(text, _)| *text).collect()
}

#[cfg(not(target_arch = "wasm32"))]
fn motd(spans: &[crate::text::Span], colour: bool) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for mut line in pieces(spans) {
        trim(&mut line);
        if !plain(&line).is_empty() {
            out.push(draw(&line, colour));
        }
    }
    if out.is_empty() {
        out.push(String::new());
    }
    out
}

#[cfg(not(target_arch = "wasm32"))]
fn draw(line: &[Piece<'_>], colour: bool) -> String {
    let mut out = String::new();
    for (text, style) in line {
        if colour && !text.is_empty() {
            out.push_str(&escape(style));
            out.push_str(text);
            out.push_str("\x1b[0m");
        } else {
            out.push_str(text);
        }
    }
    out
}

#[cfg(not(target_arch = "wasm32"))]
fn wrap(names: &[(String, String)], width: usize) -> Vec<String> {
    let mut lines: Vec<(usize, String)> = Vec::new();
    for (plain, shown) in names {
        let len = plain.chars().count();
        match lines.last_mut() {
            Some((so_far, line)) if *so_far + 2 + len <= width => {
                *so_far += 2 + len;
                line.push_str(", ");
                line.push_str(shown);
            }
            _ => lines.push((len, shown.clone())),
        }
    }
    lines.into_iter().map(|(_, line)| line).collect()
}

#[cfg(not(target_arch = "wasm32"))]
fn escape(style: &crate::text::Style) -> String {
    let mut codes = nearest(style.color).to_string();
    for (on, code) in [
        (style.bold, "1"),
        (style.italic, "3"),
        (style.underline, "4"),
        (style.strikethrough, "9"),
    ] {
        if on {
            codes.push(';');
            codes.push_str(code);
        }
    }
    format!("\x1b[{codes}m")
}

#[cfg(not(target_arch = "wasm32"))]
fn nearest(rgb: u32) -> u32 {
    const ANSI: [u32; 16] = [
        30, 34, 32, 36, 31, 35, 33, 37, 90, 94, 92, 96, 91, 95, 93, 97,
    ];
    let channels = |c: u32| [(c >> 16) & 0xFF, (c >> 8) & 0xFF, c & 0xFF];
    let want = channels(rgb);
    let mut best = (u32::MAX, 37);
    for (i, candidate) in crate::text::FORMAT_COLORS.iter().enumerate() {
        let have = channels(*candidate);
        let distance = (0..3)
            .map(|c| want[c].abs_diff(have[c]).pow(2))
            .sum::<u32>();
        if distance < best.0 {
            best = (distance, ANSI[i]);
        }
    }
    best.1
}

#[cfg(not(target_arch = "wasm32"))]
fn removable(dir: &std::path::Path) -> Result<(), String> {
    use std::path::Component;

    if !dir.is_absolute() {
        return Err(format!("{} is not an absolute path.", dir.display()));
    }
    let depth = dir
        .components()
        .filter(|c| matches!(c, Component::Normal(_)))
        .count();
    if depth < 2 {
        return Err(format!(
            "Refusing to delete {}: too close to the root of the filesystem.",
            dir.display()
        ));
    }
    for var in ["HOME", "USERPROFILE"] {
        if let Some(home) = std::env::var_os(var)
            && std::path::Path::new(&home) == dir
        {
            return Err(format!(
                "Refusing to delete {}, a home directory.",
                dir.display()
            ));
        }
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn confirm(assume_yes: bool) -> Result<bool, String> {
    use std::io::{BufRead, IsTerminal, Write};

    if assume_yes {
        return Ok(true);
    }
    if !std::io::stdin().is_terminal() {
        return Err(
            "Nothing to ask on: stdin is not a terminal. Pass --yes to delete without asking."
                .to_string(),
        );
    }
    print!("Delete these? [y/N] ");
    let _ = std::io::stdout().flush();
    let mut answer = String::new();
    if std::io::stdin().lock().read_line(&mut answer).is_err() {
        return Ok(false);
    }
    let answer = answer.trim().to_ascii_lowercase();
    Ok(answer == "y" || answer == "yes")
}

#[cfg(test)]
mod tests {
    use super::{Action, Command, Scope, parse_from};

    fn parse(args: &[&str]) -> Result<(Option<String>, Option<String>), String> {
        parse_from(args.iter().map(|a| a.to_string())).map(|c| match c {
            Command::Play(args) => (args.username, args.address),
            Command::Run(_) => (None, None),
        })
    }

    fn action(args: &[&str]) -> Result<Option<Action>, String> {
        parse_from(args.iter().map(|a| a.to_string())).map(|c| match c {
            Command::Play(_) => None,
            Command::Run(action) => Some(action),
        })
    }

    fn reset(scope: Scope) -> Option<Action> {
        Some(Action::Reset {
            scope,
            assume_yes: false,
        })
    }

    #[test]
    fn no_arguments_is_the_title_screen() {
        assert_eq!(parse(&[]), Ok((None, None)));
    }

    #[test]
    fn one_positional_is_the_address() {
        assert_eq!(
            parse(&["localhost:25565"]),
            Ok((None, Some("localhost:25565".into())))
        );
    }

    #[test]
    fn two_positionals_are_the_username_then_the_address() {
        assert_eq!(
            parse(&["Steve", "play.example"]),
            Ok((Some("Steve".into()), Some("play.example".into())))
        );
    }

    #[test]
    fn flags_win_over_positionals() {
        assert_eq!(
            parse(&["-u", "Alex", "play.example"]),
            Ok((Some("Alex".into()), Some("play.example".into())))
        );
        assert_eq!(
            parse(&["--ip", "a.example", "-u", "Alex"]),
            Ok((Some("Alex".into()), Some("a.example".into())))
        );
        assert_eq!(
            parse(&["--username", "Alex", "--address", "a.example"]),
            Ok((Some("Alex".into()), Some("a.example".into())))
        );
    }

    #[test]
    fn an_unusable_name_is_refused_before_connecting() {
        assert!(parse(&["ab", "play.example"]).is_err());
        assert!(parse(&["-u", "has a space"]).is_err());
        assert!(parse(&["-u", "seventeen_chars_x"]).is_err());
    }

    #[test]
    fn options_are_reported_rather_than_guessed_at() {
        assert!(parse(&["--nope"]).is_err());
        assert!(parse(&["-u"]).is_err());
        assert!(parse(&["a", "b", "c"]).is_err());
    }

    #[test]
    fn actions_are_recognised() {
        assert_eq!(
            action(&["--ping", "play.example"]),
            Ok(Some(Action::Ping("play.example".into())))
        );
        assert_eq!(action(&["--dir"]), Ok(Some(Action::Dir)));
        assert_eq!(action(&["--env"]), Ok(Some(Action::Env)));
        assert_eq!(action(&["--reset"]), Ok(reset(Scope::All)));
        assert_eq!(
            action(&["--reset", "--yes"]),
            Ok(Some(Action::Reset {
                scope: Scope::All,
                assume_yes: true
            }))
        );
        assert_eq!(action(&["--reset-config"]), Ok(reset(Scope::Config)));
        assert_eq!(action(&["--reset-assets"]), Ok(reset(Scope::Assets)));
        assert_eq!(action(&["play.example"]), Ok(None));
    }

    #[test]
    fn an_action_is_the_whole_command_line() {
        assert!(action(&["--dir", "play.example"]).is_err());
        assert!(action(&["--reset", "--reset-assets"]).is_err());
        assert!(action(&["--reset", "-u", "Steve"]).is_err());
        assert!(action(&["--ping", "a.example", "b.example"]).is_err());
        assert!(action(&["--ping"]).is_err());
        assert_eq!(
            action(&["--ping", "wss://play.example"]).is_err(),
            !cfg!(any(feature = "eagler", target_arch = "wasm32"))
        );
        assert_eq!(action(&["--dir", "--dir"]), Ok(Some(Action::Dir)));
    }

    #[test]
    fn yes_belongs_to_a_reset() {
        assert!(parse_from(["--reset", "--yes"].iter().map(|a| a.to_string())).is_ok());
        assert!(parse_from(["--dir", "--yes"].iter().map(|a| a.to_string())).is_err());
        assert!(parse_from(["-y", "play.example"].iter().map(|a| a.to_string())).is_err());
    }

    #[test]
    fn a_token_needs_a_value_and_a_build_that_can_use_it() {
        let parsed = parse_from(["--access-token", "abc"].iter().map(|a| a.to_string()));
        assert_eq!(parsed.is_ok(), cfg!(feature = "online_mode"));
        assert!(parse_from(["--access-token", "  "].iter().map(|a| a.to_string())).is_err());
        assert!(parse_from(["-t"].iter().map(|a| a.to_string())).is_err());
    }
}
