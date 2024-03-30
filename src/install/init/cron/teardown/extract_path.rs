use std::path::PathBuf;
use std::str::{Chars, FromStr};

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

    let command = command.trim_start();
    let command = split_unescaped_whitespace_once(command).unwrap();

    Ok(PathBuf::from_str(&command).expect("infallible"))
}

#[derive(Debug)]
struct UnescapeError;

/// **works only for paths!**
/// returns only the split off piece
fn split_unescaped_whitespace_once(line: &str) -> Result<String, UnescapeError> {
    if line.chars().count() <= 3 {
        // can not have an escaped space in an escaped path of 3
        // or less chars, example: '/ a' is the escaped path to a file
        // named space a. The escape quotes make the string 5 long.
        // Escaping with a backslash adds only one char still making
        // the path longer then 3.
        return Ok(line.to_string());
    }

    let mut chars = line.chars();
    let mut head = [
        chars.next().expect("just asserted len"),
        chars.next().expect("just asserted len"),
        chars.next().expect("just asserted len"),
    ];

    let mut out = String::with_capacity(line.len());
    let mut in_quoted = false;
    loop {
        let eaten = eat_head(head, &mut out, &mut in_quoted);
        if !in_quoted {
            let tail = &out[out.len() - eaten - 1..];
            if let Some(rel_idx) = tail.find(char::is_whitespace) {
                let _ = out.split_off(out.len() - eaten - 1 + rel_idx);
                return Ok(out);
            }
        }

        if let Err(leftover) = advance(&mut head, &mut chars, eaten) {
            final_meal(leftover, &mut out);
            return Ok(out);
        }
    }
}

fn final_meal(head: &[char], out: &mut String) {
    assert!(head.len() <= 3);
    let mut fake_head = ['_'; 3];
    fake_head[3 - head.len()..].copy_from_slice(head);
    dbg!(fake_head);
    eat_head(fake_head, out, &mut false);
}

fn eat_head(head: [char; 3], out: &mut String, in_quoted: &mut bool) -> usize {
    const QUOTE: char = '\'';
    const ESCAPE: char = '\\';

    let (unescaped_quote, eaten) = match head {
        [QUOTE, QUOTE, QUOTE] => (true, 0),
        [QUOTE, QUOTE, a] => {
            out.push(a);
            (false, 1)
        }
        [QUOTE, a, b] => {
            out.extend([a, b]);
            (true, 2)
        }
        [ESCAPE, QUOTE, QUOTE] => {
            out.extend([ESCAPE, QUOTE]);
            (true, 2)
        }
        [ESCAPE, QUOTE, _] => {
            out.extend(head);
            (false, 1)
        }
        [ESCAPE, ESCAPE, QUOTE] => {
            out.extend([ESCAPE, ESCAPE]);
            (true, 2)
        }
        [a, QUOTE, QUOTE] => {
            out.push(a);
            (false, 1)
        }
        [_, ESCAPE, QUOTE] => {
            out.extend(head);
            (false, 3)
        }
        [a, b, QUOTE] => {
            out.extend([a, b]);
            (true, 2)
        }
        [_, _, _] => {
            out.extend(head);
            (false, 3)
        }
    };

    if unescaped_quote {
        *in_quoted = !*in_quoted
    }
    eaten
}

/// returns Err(chars to process);
fn advance<'a>(head: &'a mut [char; 3], chars: &mut Chars, n: usize) -> Result<(), &'a [char]> {
    assert!(n <= head.len(), "may not skip chars in the head");
    for i in 0..n {
        let Some(next) = chars.next() else {
            return Err(&head[2 - i..]);
        };
        head[0] = head[1];
        head[1] = head[2];
        head[2] = next;
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    use std::borrow::Cow;

    #[test]
    fn eat() {
        let mut out = String::new();
        eat_head(['\'','a', 'b'] , &mut out, &mut false);
        assert_eq!(out, String::from("ab"))
    }

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
    //
    // #[test]
    // fn contains_single_quote() {
    //     check(".local/hi' there/exe", ".local/hi' there/exe")
    // }
}
