use std::borrow::Cow;

type CharIter<'a> = std::str::Chars<'a>;

fn unicode_char_from_char_digits(
    n: u8,
    base: u32,
    chars: &mut CharIter,
) -> Result<char, UnquoteError> {
    let mut num = 0;
    for i in 0..n {
        let char = chars.next().ok_or(UnquoteError::EscapeTooShort {
            expected: n,
            got: i,
        })?;
        let digit = char
            .to_digit(base)
            .ok_or(UnquoteError::DoesNotFitBase { base, char })?;
        num *= 16;
        num += digit;
    }
    char::from_u32(num).ok_or(UnquoteError::InvalidUnicode(num))
}

fn escape_hex2(chars: &mut CharIter) -> Result<char, UnquoteError> {
    unicode_char_from_char_digits(2, 16, chars)
}
fn escape_hex4(chars: &mut CharIter) -> Result<char, UnquoteError> {
    unicode_char_from_char_digits(4, 16, chars)
}
fn escape_hex8(chars: &mut CharIter) -> Result<char, UnquoteError> {
    unicode_char_from_char_digits(8, 16, chars)
}
fn escape_oct2(chars: &mut CharIter) -> Result<char, UnquoteError> {
    unicode_char_from_char_digits(2, 8, chars)
}

#[derive(Debug, thiserror::Error)]
pub enum UnquoteError {
    #[error(
        "Escape sequence ({0}) is unknown, see Table 1 in `man 7
        systemd.syntax` for the supported escape codes"
    )]
    UnknownEscape(char),
    #[error("Escaped sequence is too short, expected {expected} chars got {got}")]
    EscapeTooShort { expected: u8, got: u8 },
    #[error(
        "Sequence contains a digit ({char}) that does not fit the base
        ({base}) corrosponding to this escape sequence."
    )]
    DoesNotFitBase { base: u32, char: char },
    #[error("The digit ({0}) encoded by the escaped sequence is not a valid unicode")]
    InvalidUnicode(u32),
}

/// Attempt at getting binary path/name from systemd Exec line, effectively turns:
/// `"cmd \" with space" arg1 arg2`
/// into
/// `cmd \" with space`
/// while not perfect for undoing systemd escape its a good start
pub fn first_segement(line: &str) -> Result<Cow<str>, UnquoteError> {
    let Some(line) = line.strip_prefix('"') else {
        return Ok(Cow::Borrowed(
            line.split(' ')
                .next()
                .expect("split always returns at least one item"),
        ));
    };
    let line = dbg!(line.strip_suffix('"')).unwrap_or(line);
    let mut chars = line.chars();
    let escapes_length_one = [
        ('a', '\u{0007}'), // bell
        ('b', '\u{0008}'), // backspace
        ('f', '\u{000C}'), // form feed
        ('n', '\n'),       // newline
        ('r', '\r'),       // carriage return
        ('t', '\t'),       // tab
        ('v', '\u{000B}'), // vertical tab
        ('\\', '\\'),      // backslash
        ('"', '"'),        // double quotation mark
        ('\'', '\''),      // single quotation mark
        ('s', ' '),        // space
    ];

    let escapes_longer_then_one = [
        (
            'x',
            escape_hex2 as fn(&mut CharIter) -> Result<char, UnquoteError>,
        ),
        ('n', escape_oct2),
        ('u', escape_hex4),
        ('U', escape_hex8),
    ];

    let Some(mut a) = chars.next() else {
        return Ok(Cow::Owned(String::new()));
    };
    let mut output = String::new();
    loop {
        let Some(b) = chars.next() else {
            return Ok(Cow::Owned(output));
        };

        if a == '\\' {
            if let Some((_, unescaped)) =
                escapes_length_one.iter().find(|(literal, _)| *literal == b)
            {
                output.push(*unescaped);
                let _ = chars.by_ref().skip(1).count();
            } else if let Some((_, unescaper)) = escapes_longer_then_one
                .iter()
                .find(|(literal, _)| *literal == b)
            {
                output.push(unescaper(chars.by_ref())?);
            } else {
                return Err(UnquoteError::UnknownEscape(b));
            }
        } else if a == '"' {
            return Ok(Cow::Owned(output));
        }
        a = b;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::install::init::SystemdEscape;

    macro_rules! escaped_sequences {
        ($input:literal, $expected_output:literal, $test_name:ident) => {
            #[test]
            fn $test_name() {
                let input = $input;
                let output = first_segement(input).unwrap();
                assert_eq!(output, $expected_output);
            }
        };
    }

    escaped_sequences! {"0x69", "1", hex2}

    macro_rules! first_segement_test {
        ($test_case:literal, $test_name:ident) => {
            #[test]
            fn $test_name() {
                let input = $test_case;
                let escaped = input.systemd_escape();
                eprintln!("escaped: {escaped}");
                let path = first_segement(&escaped).unwrap();
                assert_eq!(path, input);
            }
        };
    }

    first_segement_test! {"/long/path with spaces/to/cmd", spaces}
    first_segement_test! {"v", single_letter}
    first_segement_test! {"/long              spaces/cmd", long_spaces}
    first_segement_test! {"///////cmd", many_slashes}
    first_segement_test! {"/strange\'name\"", name_with_quotes}

    #[test]
    fn hex() {
        // Hex 70 is ascii P
        // Hex 20 is ascii space
        let escaped = "\"/long/\\x70ath/withx20spacesx20/to/cmd\"";
        let cmd = first_segement(&escaped).unwrap();
        assert_eq!(cmd, "/long/path with spaces/to/cmd");
    }

    #[test]
    fn multiple_quoted_parts() {
        // Hex 70 is ascii P
        // Hex 20 is ascii space
        let escaped = "\"/path with spaces/cmd\" \"arg with quotes\" arg_without_quotes";
        let cmd = first_segement(&escaped).unwrap();
        assert_eq!(cmd, "/path with spaces/cmd");
    }
}
