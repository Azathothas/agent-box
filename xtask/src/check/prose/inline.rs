//! Inline text for the prose check: code spans, links, words, sentence ends
//! and the banned words.
//!
//! The rules: a code span is one word, and the check never reads its
//! content. A Markdown link counts its text only. A bare URL is one word. A
//! word is a token between whitespace with at least one ASCII letter or
//! digit. A sentence ends at a full stop, a question mark or an exclamation
//! mark before whitespace or the end of the unit. So the dots in a version
//! number or in a file name do not end a sentence.

/// The longest sentence that the check allows, in words.
pub(super) const LIMIT: usize = 25;

/// How many words of a long sentence the report shows.
const SHOWN: usize = 8;

/// The mark that takes the place of a code span or a URL in the text that
/// the banned-word rule reads. It is not a letter, so it is a word edge.
const MARK: char = '\u{1}';

/// Characters after a sentence end that do not hide it, as in "(here.)".
const CLOSERS: [char; 6] = [')', ']', '"', '\'', '*', '_'];

/// The words and phrases that ASD-STE100 does not allow in this repository,
/// with what to write instead. The check finds each one as a whole word, in
/// any case.
pub(super) const BANNED: [(&str, &str); 21] = [
    ("simply", "delete it"),
    ("just", "write \"only\", or delete it"),
    ("obviously", "delete it"),
    ("of course", "delete it"),
    ("very", "give a measurement, or delete it"),
    ("ensure", "write \"make sure\""),
    ("ensures", "write \"makes sure\""),
    ("ensured", "write \"made sure\""),
    ("utilize", "write \"use\""),
    ("utilise", "write \"use\""),
    ("leverage", "write \"use\""),
    ("leverages", "write \"uses\""),
    ("robust", "give the measured property"),
    ("seamless", "delete it"),
    ("seamlessly", "delete it"),
    ("load-bearing", "say what depends on it"),
    ("bulletproof", "give the measured property"),
    ("surgical", "write \"small\" or \"exact\""),
    ("e.g.", "write \"for example\""),
    ("i.e.", "write \"that is\""),
    ("etc.", "name the items"),
];

/// One piece of inline text, with the line it comes from.
#[derive(Debug, Clone)]
enum Elem {
    /// A character that can be Markdown syntax.
    Ch(char, usize),
    /// A character after a backslash. It is never syntax.
    Lit(char, usize),
    /// A code span, with its content.
    Code(String, usize),
}

impl Elem {
    fn line(&self) -> usize {
        match self {
            Elem::Ch(_, line) | Elem::Lit(_, line) | Elem::Code(_, line) => *line,
        }
    }

    /// The character, when this is not a code span.
    fn as_char(&self) -> Option<char> {
        match self {
            Elem::Ch(c, _) | Elem::Lit(c, _) => Some(*c),
            Elem::Code(..) => None,
        }
    }

    /// True when this is the syntax character `c`.
    fn is_syntax(&self, c: char) -> bool {
        matches!(self, Elem::Ch(x, _) if *x == c)
    }
}

/// One word of a unit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Word {
    /// The line where the word starts.
    pub(super) line: usize,
    /// The word as the reader sees it.
    pub(super) shown: String,
    /// The text that the banned-word rule reads.
    plain: String,
    /// True when the word counts toward the length of its sentence.
    pub(super) counts: bool,
    /// True when the word ends its sentence.
    ends: bool,
}

/// The words of one unit of text. Each character comes with its line.
pub(super) fn words(text: &[(char, usize)]) -> Vec<Word> {
    let elems = strip_links(&code_spans(text));
    elems.split(|e| e.as_char().is_some_and(char::is_whitespace)).filter(|token| !token.is_empty()).map(word).collect()
}

/// Find the code spans and the backslash escapes.
fn code_spans(text: &[(char, usize)]) -> Vec<Elem> {
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(&(c, line)) = text.get(i) {
        if c == '\\'
            && let Some(&(next, _)) = text.get(i + 1)
            && next.is_ascii_punctuation()
        {
            out.push(Elem::Lit(next, line));
            i += 2;
            continue;
        }
        if c != '`' {
            out.push(Elem::Ch(c, line));
            i += 1;
            continue;
        }
        let run = backticks(text, i);
        if let Some(close) = closing_run(text, i + run, run) {
            let content: String = text[i + run..close].iter().map(|&(ch, _)| ch).collect();
            out.push(Elem::Code(content.trim().to_string(), line));
            i = close + run;
        } else {
            // No closing run: the backticks are text.
            out.extend(text[i..i + run].iter().map(|&(ch, l)| Elem::Ch(ch, l)));
            i += run;
        }
    }
    out
}

/// The length of the run of backticks that starts at `start`.
fn backticks(text: &[(char, usize)], start: usize) -> usize {
    text.get(start..).unwrap_or_default().iter().take_while(|&&(c, _)| c == '`').count()
}

/// The start of the next run of exactly `run` backticks at or after `from`.
fn closing_run(text: &[(char, usize)], from: usize, run: usize) -> Option<usize> {
    let mut j = from;
    while let Some(&(c, _)) = text.get(j) {
        if c == '`' {
            let len = backticks(text, j);
            if len == run {
                return Some(j);
            }
            j += len;
        } else {
            j += 1;
        }
    }
    None
}

/// Replace each link with its text. An image is a link here, with its alt
/// text as the link text.
fn strip_links(elems: &[Elem]) -> Vec<Elem> {
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(elem) = elems.get(i) {
        let open = if elem.is_syntax('[') {
            Some(i)
        } else if elem.is_syntax('!') && elems.get(i + 1).is_some_and(|e| e.is_syntax('[')) {
            Some(i + 1)
        } else {
            None
        };
        if let Some(open) = open
            && let Some((text, next)) = link(elems, open)
        {
            out.extend(strip_links(text));
            i = next;
            continue;
        }
        out.push(elem.clone());
        i += 1;
    }
    out
}

/// The link that starts at the `[` at `open`: its text, and the index after
/// it. Only an inline link `[text](target)` or a full reference link
/// `[text][label]` is a link here.
fn link(elems: &[Elem], open: usize) -> Option<(&[Elem], usize)> {
    let close = matching(elems, open, '[', ']')?;
    let end = match elems.get(close + 1) {
        Some(e) if e.is_syntax('(') => matching(elems, close + 1, '(', ')')?,
        Some(e) if e.is_syntax('[') => matching(elems, close + 1, '[', ']')?,
        _ => return None,
    };
    Some((&elems[open + 1..close], end + 1))
}

/// The index of the bracket that closes the bracket at `open`.
fn matching(elems: &[Elem], open: usize, left: char, right: char) -> Option<usize> {
    let mut depth = 0_usize;
    for (index, e) in elems.iter().enumerate().skip(open) {
        if e.is_syntax(left) {
            depth += 1;
        } else if e.is_syntax(right) {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return Some(index);
            }
        }
    }
    None
}

/// One token between whitespace, as a word.
fn word(token: &[Elem]) -> Word {
    let mut shown = String::new();
    let mut plain = String::new();
    let mut letters = false;
    let mut code = false;
    for e in token {
        match e {
            Elem::Ch(c, _) | Elem::Lit(c, _) => {
                shown.push(*c);
                plain.push(*c);
                letters |= c.is_ascii_alphanumeric();
            }
            Elem::Code(content, _) => {
                shown.push('`');
                shown.push_str(content);
                shown.push('`');
                plain.push(MARK);
                code = true;
            }
        }
    }
    let url = is_url(&shown);
    if url {
        plain = MARK.to_string();
    }
    Word { line: token.first().map_or(0, Elem::line), shown, plain, counts: letters || code || url, ends: ends(token) }
}

fn is_url(token: &str) -> bool {
    let token = token.trim_start_matches(['<', '(', '"', '\'']);
    token.contains("://") || token.starts_with("www.") || token.starts_with("mailto:")
}

/// True when the token ends a sentence: its last character, before any
/// closing marks, is a full stop, a question mark or an exclamation mark.
fn ends(token: &[Elem]) -> bool {
    let last = token.iter().rev().find(|e| !e.as_char().is_some_and(|c| CLOSERS.contains(&c)));
    last.and_then(Elem::as_char).is_some_and(|c| matches!(c, '.' | '?' | '!'))
}

/// A sentence over the limit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Long {
    /// The line of its first word.
    pub(super) line: usize,
    /// Its length in words.
    pub(super) count: usize,
    /// Its first words, as the reader sees them.
    pub(super) start: String,
}

/// The sentences of a unit that have more words than the limit.
pub(super) fn long_sentences(words: &[Word]) -> Vec<Long> {
    let mut out = Vec::new();
    for sentence in words.split_inclusive(|w| w.ends) {
        let counted: Vec<&Word> = sentence.iter().filter(|w| w.counts).collect();
        if let Some(first) = counted.first()
            && counted.len() > LIMIT
        {
            let start: Vec<&str> = counted.iter().take(SHOWN).map(|w| w.shown.as_str()).collect();
            out.push(Long { line: first.line, count: counted.len(), start: start.join(" ") });
        }
    }
    out
}

/// One banned word in a unit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Hit {
    pub(super) line: usize,
    pub(super) term: &'static str,
    pub(super) hint: &'static str,
}

/// The banned words of a unit, in line order.
pub(super) fn banned(words: &[Word]) -> Vec<Hit> {
    // One string for the unit, with one space between words, so that a
    // phrase of two words matches across a line end.
    let mut plain = String::new();
    let mut starts = Vec::with_capacity(words.len());
    for w in words {
        if !plain.is_empty() {
            plain.push(' ');
        }
        starts.push(plain.len());
        plain.push_str(&w.plain);
    }
    plain.make_ascii_lowercase();
    let bytes = plain.as_bytes();
    let mut hits = Vec::new();
    for (term, hint) in BANNED {
        for (at, _) in plain.match_indices(term) {
            if edge_before(bytes, at) && edge_after(bytes, at + term.len()) {
                let index = starts.partition_point(|&s| s <= at).saturating_sub(1);
                hits.push(Hit { line: words.get(index).map_or(0, |w| w.line), term, hint });
            }
        }
    }
    hits.sort_by_key(|h| h.line);
    hits
}

/// True for a byte that joins two parts of one token: a letter, a digit, a
/// hyphen, a slash, a backslash or an at sign. So a match inside a longer
/// word, a hyphenated word, a path or an address is not a whole word.
fn joins(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'-' | b'/' | b'\\' | b'@') || !b.is_ascii()
}

/// True when a match that starts at `at` has a word edge before it. A dot
/// with a letter or a digit before it joins too, as in a file name.
fn edge_before(text: &[u8], at: usize) -> bool {
    let Some(&prev) = at.checked_sub(1).and_then(|i| text.get(i)) else {
        return true;
    };
    let dotted = prev == b'.' && at.checked_sub(2).and_then(|i| text.get(i)).is_some_and(u8::is_ascii_alphanumeric);
    !joins(prev) && !dotted
}

/// True when a match that ends before `end` has a word edge after it. A dot
/// with a letter or a digit after it joins too, as in a file name.
fn edge_after(text: &[u8], end: usize) -> bool {
    let Some(&next) = text.get(end) else {
        return true;
    };
    let dotted = next == b'.' && text.get(end + 1).is_some_and(u8::is_ascii_alphanumeric);
    !joins(next) && !dotted
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(text: &str) -> Vec<(char, usize)> {
        text.chars().map(|c| (c, 1)).collect()
    }

    fn count(text: &str) -> usize {
        words(&unit(text)).iter().filter(|w| w.counts).count()
    }

    fn hits(text: &str) -> Vec<&'static str> {
        banned(&words(&unit(text))).into_iter().map(|h| h.term).collect()
    }

    fn ends_of(text: &str) -> Vec<bool> {
        words(&unit(text)).iter().map(|w| w.ends).collect()
    }

    #[test]
    fn a_code_span_is_one_word() {
        assert_eq!(count("run `cargo xtask check prose` now"), 3);
        assert_eq!(count("a ``code with ` inside`` b"), 3);
        assert_eq!(count("`-` and `|`"), 3);
    }

    #[test]
    fn a_backtick_without_a_closing_run_is_text() {
        assert_eq!(count("a ` b c"), 3);
    }

    #[test]
    fn a_link_counts_its_text_only() {
        assert_eq!(count("see [the two words](docs/a.md#some-long-anchor) here"), 5);
        assert_eq!(count("see [text][label] and ![alt text](image.png)"), 5);
        assert_eq!(count("a [`code` link](x.md)."), 3);
    }

    #[test]
    fn a_bare_url_is_one_word() {
        assert_eq!(count("see https://example.com/a-b/c?d=e now"), 3);
        assert_eq!(count("see <https://example.com/a> now"), 3);
    }

    #[test]
    fn punctuation_alone_is_not_a_word() {
        assert_eq!(count("a - b -> c | d"), 4);
    }

    #[test]
    fn a_version_number_or_a_file_name_does_not_end_a_sentence() {
        assert_eq!(ends_of("Rust 1.99.0 reads a.rs today. Next"), [false, false, false, false, true, false]);
    }

    #[test]
    fn a_question_an_exclamation_and_a_closed_end_end_a_sentence() {
        assert_eq!(ends_of("Why? Now! (here.) **Bold.** `code.` x"), [true, true, true, true, false, false]);
    }

    #[test]
    fn an_escaped_bracket_is_not_a_link() {
        // Three tokens: "[a", "b](c" and "d)". As a link, the count is two.
        assert_eq!(count("\\[a b\\](c d)"), 3);
    }

    #[test]
    fn each_banned_word_is_found_in_any_case() {
        for (term, _) in BANNED {
            assert_eq!(hits(&format!("We {term} go.")), [term]);
            assert_eq!(hits(&format!("({}) go.", term.to_ascii_uppercase())), [term]);
        }
    }

    #[test]
    fn a_banned_word_inside_a_longer_word_is_not_found() {
        let text = "adjust, robustness, justify, /usr/etc/x, a just-in-time build, file.just and just.rs";
        assert_eq!(hits(text), Vec::<&str>::new());
    }

    #[test]
    fn a_banned_word_in_code_or_in_a_url_is_not_found() {
        assert_eq!(hits("use `just` and https://example.com/just/simply"), Vec::<&str>::new());
        assert_eq!(hits("[just here](just.md)"), ["just"]);
    }

    #[test]
    fn a_phrase_matches_across_a_line_end() {
        let mut text = unit("Of");
        text.push((' ', 1));
        text.extend("course".chars().map(|c| (c, 2)));
        let found = banned(&words(&text));
        assert_eq!(found.len(), 1);
        assert_eq!((found[0].line, found[0].term), (1, "of course"));
        assert_eq!(hits("of `x` course"), Vec::<&str>::new());
    }

    #[test]
    fn a_long_sentence_shows_its_length_and_first_words() {
        let text: Vec<String> = (1..=30).map(|n| format!("w{n}")).collect();
        let long = long_sentences(&words(&unit(&text.join(" "))));
        assert_eq!(long, [Long { line: 1, count: 30, start: "w1 w2 w3 w4 w5 w6 w7 w8".to_string() }]);
    }
}
