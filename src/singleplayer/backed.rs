#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Control {
    Hardcore,
    AllowCommands,
    WorldType,
    CustomizeType,
    Seed,
    GenerateStructures,
    BonusChest,
    GameRules,
    Experiments,
    DataPacks,
    Difficulty,
    GameMode,
    Backup,
    Optimize,
    OpenToLan,
    Recreate,
    ResetIcon,
}

pub(crate) const fn is_backed(c: Control) -> bool {
    matches!(
        c,
        Control::GameMode
            | Control::Difficulty
            | Control::AllowCommands
            | Control::Seed
            | Control::GameRules
            | Control::Recreate
            | Control::ResetIcon
            | Control::Backup
    )
}
