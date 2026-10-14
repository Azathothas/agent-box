//! The comments of a Rust source file, for the prose check.
//!
//! A small lexer. It knows string literals, raw strings, character literals
//! and lifetimes, so a comment opener inside a string is not a comment, and a
//! quote character in a character literal does not start a string.

/// The text of each comment line, with its line number. The comment opener,
/// a doc mark and the `*` at the start of a block comment line are not part
/// of the text.
pub(super) fn comments(source: &str) -> Vec<(usize, String)> {
    let chars: Vec<char> = source.chars().collect();
    let mut lexer = Lexer { chars: &chars, at: 0, line: 1, out: Vec::new() };
    lexer.run();
    lexer.out
}

#[derive(Debug)]
struct Lexer<'a> {
    chars: &'a [char],
    at: usize,
    line: usize,
    out: Vec<(usize, String)>,
}

impl Lexer<'_> {
    fn peek(&self, ahead: usize) -> Option<char> {
        self.chars.get(self.at + ahead).copied()
    }

    /// Move one character forward, and count a line end.
    fn bump(&mut self) {
        if self.peek(0) == Some('\n') {
            self.line += 1;
        }
        self.at += 1;
    }

    fn run(&mut self) {
        while let Some(c) = self.peek(0) {
            match (c, self.peek(1)) {
                ('/', Some('/')) => self.line_comment(),
                ('/', Some('*')) => self.block_comment(),
                ('"', _) => {
                    self.at += 1;
                    self.string();
                }
                ('\'', _) => self.quote(),
                ('r', _) => {
                    if let Some(hashes) = self.raw_hashes() {
                        self.raw_string(hashes);
                    } else {
                        self.bump();
                    }
                }
                _ => self.bump(),
            }
        }
    }

    fn line_comment(&mut self) {
        let start = self.at + 2;
        let rest = self.chars.get(start..).unwrap_or_default();
        let len = rest.iter().take_while(|&&c| c != '\n').count();
        let text: String = rest[..len].iter().collect();
        self.out.push((self.line, strip_mark(&text)));
        self.at = start + len;
    }

    /// A block comment. Block comments nest in Rust.
    fn block_comment(&mut self) {
        self.at += 2;
        let mut depth = 1_usize;
        let mut text = String::new();
        while let Some(c) = self.peek(0) {
            match (c, self.peek(1)) {
                ('*', Some('/')) => {
                    self.at += 2;
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                    text.push_str("*/");
                }
                ('/', Some('*')) => {
                    self.at += 2;
                    depth += 1;
                    text.push_str("/*");
                }
                ('\n', _) => {
                    self.out.push((self.line, strip_mark(&text)));
                    text.clear();
                    self.bump();
                }
                _ => {
                    text.push(c);
                    self.at += 1;
                }
            }
        }
        self.out.push((self.line, strip_mark(&text)));
    }

    /// The rest of a string literal, after its opening quote.
    fn string(&mut self) {
        while let Some(c) = self.peek(0) {
            match c {
                '\\' => {
                    // The escaped character can be a line end.
                    self.at += 1;
                    self.bump();
                }
                '"' => {
                    self.at += 1;
                    return;
                }
                _ => self.bump(),
            }
        }
    }

    /// The number of hashes when a raw string starts at the `r` here. The `r`
    /// must start a token, or follow a `b` or a `c` that starts a token.
    fn raw_hashes(&self) -> Option<usize> {
        let before = |back: usize| self.at.checked_sub(back).and_then(|i| self.chars.get(i)).copied();
        let starts_token = match before(1) {
            None => true,
            Some('b' | 'c') => !before(2).is_some_and(is_ident),
            Some(prev) => !is_ident(prev),
        };
        if !starts_token {
            return None;
        }
        let hashes = self.chars.get(self.at + 1..).unwrap_or_default().iter().take_while(|&&c| c == '#').count();
        (self.peek(1 + hashes) == Some('"')).then_some(hashes)
    }

    fn raw_string(&mut self, hashes: usize) {
        self.at += 2 + hashes;
        while let Some(c) = self.peek(0) {
            if c == '"' && (1..=hashes).all(|k| self.peek(k) == Some('#')) {
                self.at += 1 + hashes;
                return;
            }
            self.bump();
        }
    }

    /// A character literal or a lifetime, at a `'`.
    fn quote(&mut self) {
        if self.peek(1) == Some('\\') {
            // An escape. The literal ends at the next quote on this line.
            let rest = self.chars.get(self.at + 3..).unwrap_or_default();
            let end = rest.iter().take_while(|&&c| c != '\n').position(|&c| c == '\'');
            self.at += end.map_or(1, |p| p + 4);
        } else if self.peek(2) == Some('\'') && self.peek(1) != Some('\n') {
            self.at += 3;
        } else {
            // A lifetime or a loop label.
            self.at += 1;
        }
    }
}

fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// The comment text without the doc mark (`/` or `!`) or the `*` that starts
/// a line of a block comment.
fn strip_mark(text: &str) -> String {
    text.trim_start().trim_start_matches(['/', '!', '*']).trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(source: &str) -> Vec<(usize, String)> {
        comments(source).into_iter().filter(|(_, text)| !text.is_empty()).collect()
    }

    fn one(line: usize, text: &str) -> Vec<(usize, String)> {
        vec![(line, text.to_string())]
    }

    #[test]
    fn line_and_doc_comments_are_read_with_their_lines() {
        let source = "//! inner\nfn f() {}\n/// outer\nlet x = 1; // after\n";
        assert_eq!(texts(source), vec![(1, "inner".to_string()), (3, "outer".to_string()), (4, "after".to_string())]);
    }

    #[test]
    fn a_comment_opener_in_a_string_is_not_a_comment() {
        assert_eq!(texts("let a = \"// no /* no\";\n// yes\n"), one(2, "yes"));
        assert_eq!(texts("let a = \"a \\\" // no\";\n// yes\n"), one(2, "yes"));
    }

    #[test]
    fn a_raw_string_is_skipped() {
        assert_eq!(texts("let a = r#\"// no \" still\"#; // yes\n"), one(1, "yes"));
        assert_eq!(texts("let a = br\"// no\"; // yes\n"), one(1, "yes"));
        assert_eq!(texts("let r = 1; // yes\n"), one(1, "yes"));
    }

    #[test]
    fn a_quote_in_a_character_literal_does_not_start_a_string() {
        assert_eq!(texts("let q = '\"'; // yes\n"), one(1, "yes"));
        assert_eq!(texts("let q = '\\''; let r = b'\\\\'; // yes\n"), one(1, "yes"));
        assert_eq!(texts("let m = '\\u{1}'; // yes\n"), one(1, "yes"));
    }

    #[test]
    fn a_lifetime_is_not_a_character_literal() {
        assert_eq!(texts("fn f<'a>(x: &'a str) -> &'a str { x } // yes\n"), one(1, "yes"));
        assert_eq!(texts("impl Lexer<'_> {} // yes\n"), one(1, "yes"));
    }

    #[test]
    fn block_comments_nest_and_keep_their_lines() {
        let source = "/* one\n * two /* inner */\n three */ let x = \"/* no */\";\n";
        assert_eq!(
            texts(source),
            vec![(1, "one".to_string()), (2, "two /* inner */".to_string()), (3, "three".to_string())]
        );
    }
}
