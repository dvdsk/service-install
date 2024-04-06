use std::iter;

/// **works only for paths!**
/// returns only the split off piece
pub fn split_unescaped_whitespace_once(line: &str) -> String {
    if line.chars().count() <= 3 {
        // can not have an escaped space in an escaped path of 3
        // or less chars, example: '/ a' is the escaped path to a file
        // named space a. The escape quotes make the string 5 long.
        // Escaping with a backslash adds only one char still making
        // the path longer then 3.
        return line.to_string();
    }

    let mut chars = line.chars().chain(iter::repeat('_').take(3));
    let mut head = [
        '_', // padding removed at the end
        '_',
        chars.next().expect("just asserted len"),
    ];

    let mut out = String::with_capacity(line.len());
    let mut in_quoted = false;
    loop {
        let eaten = eat_head(head, &mut out, &mut in_quoted);
        if !in_quoted {
            let tail = &out[out.len().saturating_sub(eaten).saturating_sub(1)..];
            if let Some(rel_idx) = tail.find(char::is_whitespace) {
                let _ = out.split_off(out.len() - eaten - 1 + rel_idx);
                out.drain(0..2);
                return out;
            }
        }
        let done = advance(&mut head, &mut chars, eaten);
        if done {
            out.drain(0..2);
            out.pop();
            return out;
        }
    }
}

fn eat_head(head: [char; 3], out: &mut String, in_quoted: &mut bool) -> usize {
    const QUOTE: char = '\'';
    const ESCAPE: char = '\\';

    let (unescaped_quote, eaten) = match head {
        [ESCAPE, ESCAPE, QUOTE] => {
            out.push(ESCAPE);
            (true, 3)
        }
        [ESCAPE, QUOTE, _] => {
            out.push(QUOTE);
            (false, 2)
        }
        [QUOTE, _, _] => (true, 1),
        [a, _, _] => {
            out.push(a);
            (false, 1)
        }
    };

    if unescaped_quote {
        *in_quoted = !*in_quoted
    }
    eaten
}

/// returns Err(chars to process);
fn advance(head: &mut [char; 3], chars: &mut impl Iterator<Item = char>, n: usize) -> bool {
    assert!(n <= head.len(), "may not skip chars in the head");
    for _ in 0..n {
        let Some(next) = chars.next() else {
            return true;
        };
        head[0] = head[1];
        head[1] = head[2];
        head[2] = next;
    }
    false
}

#[cfg(test)]
mod test {
    use super::*;
    use std::borrow::Cow;

    #[test]
    fn eat_double_escape() {
        let mut out = String::new();
        eat_head(['\\', '\\', '\''], &mut out, &mut false);
        assert_eq!(out, String::from("\\"))
    }

    fn check(input: &'static str) {
        let escaped = shell_escape::unix::escape(Cow::Borrowed(input)).to_string();
        eprintln!("escaped: {escaped}");
        let path = split_unescaped_whitespace_once(&escaped);
        assert_eq!(&path, input);
    }

    #[test]
    fn contains_space() {
        check(".local/hi there/exe")
    }

    #[test]
    fn contains_single_quote() {
        check(".local/hi' there/exe")
    }

    #[test]
    fn realistic() {
        check("/home/david/.local/hi bin/cron_only")
    }

    #[test]
    fn smoke() {
        check("i't")
    }
}
