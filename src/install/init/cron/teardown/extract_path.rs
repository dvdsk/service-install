use std::path::PathBuf;
use std::str::FromStr;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Rule in crontab corrupt, too short")]
    CorruptTooShort,
    #[error("No rule from previous install in crontab")]
    NoRule,
    #[error("The command in crontab should not be empty/length zero")]
    EmptyCommand,
    #[error("The command is shell escaped but the second escape character is missing")]
    EscapedEndMissing,
}

pub(super) fn from_rule(rule: &str) -> Result<PathBuf, Error> {
    let command = if let Some(command) = rule.strip_prefix("@reboot") {
        command.to_string()
    } else {
        rule.split_whitespace().skip(5).collect()
    };

    const SINGLE_QUOTE: char = '\'';
    const ESCAPE: char = '\\';
    let command = command.trim_start();
    let command = if let Some(quoted) = command.strip_prefix(SINGLE_QUOTE) {
        let end = quoted
            .match_indices("' ")
            .map(|(idx, _)| idx)
            .filter_map(|idx| {
                let mut quoted = quoted.chars();
                let two_before_quote = quoted.nth(idx.saturating_sub(2));
                let one_before_quote = quoted.next();
                two_before_quote.zip(one_before_quote).zip(Some(idx))
            })
            .find(|((two_before, one_before), _)| *two_before == ESCAPE || *one_before != ESCAPE)
            .map(|(_, idx)| idx)
            .ok_or(Error::EscapedEndMissing)?;
        dbg!(quoted, end);
        &quoted[..end]
    } else {
        command
            .split_whitespace()
            .next()
            .ok_or(Error::EmptyCommand)?
    };

    Ok(PathBuf::from_str(command).expect("infallible"))
}

#[cfg(test)]
mod test {
    use super::*;
    use std::borrow::Cow;

    fn check(input: &'static str, correct: &'static str) {
        let escaped = shell_escape::unix::escape(Cow::Borrowed(input)).to_string();
        eprintln!("escaped: {escaped}");
        let rule = "@reboot ".to_string() + &escaped;
        let path = from_rule(&rule).unwrap();
        let path = path.to_string_lossy();
        assert_eq!(&path, correct);
    }

    #[test]
    fn contains_space() {
        check(".local/hi there/exe", ".local/hi there/exe")
    }

    #[test]
    fn contains_single_quote() {
        check(".local/hi' there/exe", ".local/hi' there/exe")
    }
}
