#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SlashCommand {
    Quit,
    Clear,
    New,
    Help,
    Sessions,
    Config,
}

const COMMANDS: &[(SlashCommand, &str)] = &[
    (SlashCommand::Config, "/config"),
    (SlashCommand::Help, "/help"),
    (SlashCommand::Quit, "/quit"),
    (SlashCommand::Clear, "/clear"),
    (SlashCommand::New, "/new"),
    (SlashCommand::Sessions, "/sessions"),
];

pub(crate) fn parse_slash_command(input: &str) -> Option<SlashCommand> {
    COMMANDS
        .iter()
        .find_map(|(command, spelling)| (*spelling == input).then_some(*command))
}

pub(crate) fn resolve_slash_command(input: &str) -> Option<SlashCommand> {
    parse_slash_command(input).or_else(|| {
        COMMANDS.iter().find_map(|(command, spelling)| {
            spelling
                .starts_with(input)
                .then_some(*command)
                .filter(|_| inline_suggestion_suffix(input).is_some())
        })
    })
}

pub(crate) fn inline_suggestion_suffix(input: &str) -> Option<&'static str> {
    if !input.starts_with('/') || input.contains(char::is_whitespace) {
        return None;
    }

    COMMANDS.iter().find_map(|(_, spelling)| {
        spelling
            .starts_with(input)
            .then(|| &spelling[input.len()..])
            .filter(|suffix| !suffix.is_empty())
    })
}

#[cfg(test)]
mod tests {
    use super::{
        SlashCommand, inline_suggestion_suffix, parse_slash_command, resolve_slash_command,
    };

    #[test]
    fn parses_known_commands() {
        assert_eq!(parse_slash_command("/help"), Some(SlashCommand::Help));
        assert_eq!(parse_slash_command("/quit"), Some(SlashCommand::Quit));
        assert_eq!(parse_slash_command("/clear"), Some(SlashCommand::Clear));
        assert_eq!(parse_slash_command("/new"), Some(SlashCommand::New));
        assert_eq!(parse_slash_command("/sessions"), Some(SlashCommand::Sessions));
        assert_eq!(parse_slash_command("/config"), Some(SlashCommand::Config));
    }

    #[test]
    fn suggestion_returns_only_missing_suffix() {
        assert_eq!(inline_suggestion_suffix("/"), Some("config"));
        assert_eq!(inline_suggestion_suffix("/he"), Some("lp"));
        assert_eq!(inline_suggestion_suffix("/ses"), Some("sions"));
    }

    #[test]
    fn suggestion_is_hidden_for_exact_or_invalid_inputs() {
        assert_eq!(inline_suggestion_suffix("/help"), None);
        assert_eq!(inline_suggestion_suffix("/unknown"), None);
        assert_eq!(inline_suggestion_suffix("hello"), None);
        assert_eq!(inline_suggestion_suffix("/help me"), None);
    }

    #[test]
    fn resolve_accepts_exact_and_suggested_commands() {
        assert_eq!(resolve_slash_command("/help"), Some(SlashCommand::Help));
        assert_eq!(resolve_slash_command("/he"), Some(SlashCommand::Help));
        assert_eq!(resolve_slash_command("/ses"), Some(SlashCommand::Sessions));
        assert_eq!(resolve_slash_command("/unknown"), None);
    }
}
