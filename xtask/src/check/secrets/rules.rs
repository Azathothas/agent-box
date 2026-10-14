//! The shapes that the `secrets` check looks for. Each shape is the documented
//! token format of one vendor, the header line of a private key block, a
//! credential file name, or the long-hex rule.
//!
//! A generic "high entropy" rule is absent on purpose. It fires on hashes and
//! on test data, and people switch off a check that fires on correct files.
//! The crate has no regular expression library, so each rule matches bytes.
//! Each shape is ASCII, so a match starts on a character boundary.
//!
//! NOTE: This file keeps each prefix as a separate string, so the source does
//! not match its own rules. The tests build each planted value at run time
//! for the same reason.

use std::path::Path;

use crate::repo::is_markdown;
use crate::report::Finding;

/// How many characters of a match a finding shows. The rest stays hidden.
const SHOWN: usize = 4;

/// A hex run of this length or more is a finding, unless its shape says that it is public.
const HEX_MIN: usize = 40;

/// A body with no upper limit on its length.
const ANY: usize = usize::MAX;

/// One match in a line: the rule, and the byte offset where the match starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Hit {
    rule: &'static str,
    at: usize,
}

/// The facts about a file that change what a rule accepts.
#[derive(Debug, Clone, Copy, Default)]
struct FileKind {
    /// `Cargo.lock`: each package has a checksum, so the long-hex rule is off.
    lock: bool,
    /// A Markdown document: its pins table holds full commits.
    markdown: bool,
}

impl FileKind {
    fn of(rel: &str) -> Self {
        let name = rel.rsplit('/').next().unwrap_or(rel);
        Self { lock: name == "Cargo.lock", markdown: is_markdown(rel) }
    }
}

/// The findings in the text of one file. Line numbers start at 1.
pub(super) fn scan_text(rel: &str, text: &str) -> Vec<Finding> {
    let kind = FileKind::of(rel);
    let mut out = Vec::new();
    for (index, raw) in text.split('\n').enumerate() {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        for hit in scan_line(line, kind) {
            out.push(Finding::new(rel, index + 1, format!("{}: {}", hit.rule, excerpt(line, hit.at))));
        }
    }
    out
}

/// The first characters of a match and an ellipsis. Never the whole value.
fn excerpt(line: &str, at: usize) -> String {
    let head: String = line.get(at..).unwrap_or_default().chars().take(SHOWN).collect();
    format!("{head}...")
}

/// Each match of each rule in one line, in the order of the line.
fn scan_line(line: &str, kind: FileKind) -> Vec<Hit> {
    let mut hits = Vec::new();
    private_key(line, &mut hits);
    for rule in PREFIXED {
        prefixed(line, rule, &mut hits);
    }
    sk_key(line, &mut hits);
    dotted(line, &mut hits);
    if !kind.lock {
        long_hex(line, kind, &mut hits);
    }
    hits.sort_by_key(|h| h.at);
    hits
}

/// File extensions of key files and token files.
const KEY_EXTENSIONS: [&str; 7] = ["pem", "key", "p12", "pfx", "keystore", "jks", "token"];

/// File names of private SSH keys and of cloud credentials.
const KEY_NAMES: [&str; 5] = ["id_rsa", "id_dsa", "id_ecdsa", "id_ed25519", "credentials.json"];

/// Extensions of a sample file. A sample shows the shape of a credential file and holds no value.
const SAMPLE_EXTENSIONS: [&str; 4] = ["example", "sample", "template", "dist"];

/// Why the name of a file says that it holds a secret, or `None`. The name is
/// the strongest signal: the only purpose of such a file is to hold a secret.
pub(super) fn credential_file(rel: &str) -> Option<&'static str> {
    let name = rel.rsplit('/').next().unwrap_or(rel).to_ascii_lowercase();
    let ext = Path::new(&name).extension().and_then(|e| e.to_str()).unwrap_or_default();
    if SAMPLE_EXTENSIONS.contains(&ext) {
        return None;
    }
    let env = name == ".env" || name.starts_with(".env.") || name == ".dev.vars" || name.starts_with(".dev.vars.");
    let token = name.starts_with('.') && (name.ends_with("_token") || name.ends_with("-token"));
    let cloud = name.starts_with("service-account") && ext == "json";
    let named = KEY_EXTENSIONS.contains(&ext) || KEY_NAMES.contains(&name.as_str());
    (env || token || cloud || named).then_some("a credential file: a file with this name holds a secret")
}

/// The two parts of a PEM header line, kept apart so that this file does not match the rule.
const KEY_OPEN: &str = "BEGIN ";
const KEY_TAIL: &str = "PRIVATE KEY";

/// The header line of a PEM private key block. Up to two type words can stand
/// between the two parts, for example RSA, OPENSSH or ENCRYPTED.
fn private_key(line: &str, hits: &mut Vec<Hit>) {
    let bytes = line.as_bytes();
    for (at, _) in line.match_indices(KEY_OPEN) {
        let mut next = at + KEY_OPEN.len();
        for _ in 0..=2 {
            if bytes.get(next..).is_some_and(|rest| rest.starts_with(KEY_TAIL.as_bytes())) {
                hits.push(Hit { rule: "a private key header", at });
                break;
            }
            let word_len = run(bytes, next, upper_alnum);
            if word_len == 0 || bytes.get(next + word_len) != Some(&b' ') {
                break;
            }
            next += word_len + 1;
        }
    }
}

/// A token that starts with a fixed prefix and goes on with a body of known bytes.
struct Prefixed {
    name: &'static str,
    prefixes: &'static [&'static str],
    /// The bytes that the body can hold.
    body: fn(u8) -> bool,
    /// The shortest and the longest body that the rule accepts.
    min: usize,
    max: usize,
}

/// The vendor token shapes that start with a fixed prefix.
#[rustfmt::skip]
const PREFIXED: &[Prefixed] = &[
    Prefixed { name: "an AWS access key id", prefixes: &["AKIA", "ASIA"], body: upper_alnum, min: 16, max: 16 },
    Prefixed { name: "a GitHub token", prefixes: &["ghp_", "gho_", "ghu_", "ghs_", "ghr_"], body: alnum, min: 36, max: ANY },
    Prefixed { name: "a GitHub token", prefixes: &["github_pat_"], body: word, min: 22, max: ANY },
    Prefixed { name: "a Slack token", prefixes: &["xoxb-", "xoxa-", "xoxp-", "xoxr-", "xoxs-"], body: alnum_dash, min: 10, max: ANY },
    Prefixed { name: "a Google API key", prefixes: &["AIza"], body: base64url, min: 35, max: ANY },
    Prefixed { name: "an sk-ant- API key", prefixes: &["sk-ant-"], body: base64url, min: 20, max: ANY },
    Prefixed { name: "a Stripe secret key", prefixes: &["sk_live_", "sk_test_", "rk_live_"], body: alnum, min: 16, max: ANY },
    Prefixed { name: "an npm token", prefixes: &["npm_"], body: alnum, min: 36, max: ANY },
];

/// Each match of one prefixed rule. The byte before the prefix and the byte
/// after the body cannot be a letter or a digit, so a match is a whole token.
fn prefixed(line: &str, rule: &Prefixed, hits: &mut Vec<Hit>) {
    let bytes = line.as_bytes();
    for &prefix in rule.prefixes {
        for (at, _) in line.match_indices(prefix) {
            let start = at + prefix.len();
            let len = run(bytes, start, rule.body);
            let ends = bytes.get(start + len).is_none_or(|&c| !alnum(c));
            if token_start(bytes, at) && ends && (rule.min..=rule.max).contains(&len) {
                hits.push(Hit { rule: rule.name, at });
            }
        }
    }
}

const SK: &str = "sk-";

/// An `sk-` key: a body of 32 or more bytes with digits and letters, the API
/// key shape of several vendors. An `sk-ant-` key has its own rule.
fn sk_key(line: &str, hits: &mut Vec<Hit>) {
    let bytes = line.as_bytes();
    for (at, _) in line.match_indices(SK) {
        let start = at + SK.len();
        let body = &bytes[start..start + run(bytes, start, base64url)];
        let mixed = body.iter().any(u8::is_ascii_digit) && body.iter().any(u8::is_ascii_alphabetic);
        if token_start(bytes, at) && !body.starts_with(b"ant-") && body.len() >= 32 && mixed {
            hits.push(Hit { rule: "an sk- API key", at });
        }
    }
}

/// The base64url form of the two bytes that start a JSON object.
const JWT_HEAD: &[u8] = b"eyJ";

/// A token of three base64url segments joined by dots: a JWT or a Discord bot token.
fn dotted(line: &str, hits: &mut Vec<Hit>) {
    let bytes = line.as_bytes();
    let mut pos = 0;
    while pos < bytes.len() {
        let first = run(bytes, pos, base64url);
        if first == 0 {
            pos += 1;
            continue;
        }
        // The segments of one dotted run, as (start, length).
        let mut segments = vec![(pos, first)];
        let mut end = pos + first;
        while bytes.get(end) == Some(&b'.') {
            let len = run(bytes, end + 1, base64url);
            if len == 0 {
                break;
            }
            segments.push((end + 1, len));
            end += 1 + len;
        }
        for window in segments.windows(3) {
            if let Some(rule) = dotted_rule(bytes, window) {
                hits.push(Hit { rule, at: window[0].0 });
            }
        }
        pos = end;
    }
}

/// The rule that three dotted segments match, if one does.
fn dotted_rule(bytes: &[u8], window: &[(usize, usize)]) -> Option<&'static str> {
    let &[first, second, third] = window else { return None };
    let (head, body, tail) = (segment(bytes, first), segment(bytes, second), segment(bytes, third));
    // A JWT: the header and the payload are JSON objects, and the third part is the signature.
    let jwt = head.starts_with(JWT_HEAD) && body.starts_with(JWT_HEAD);
    if jwt && head.len() >= 10 && body.len() >= 10 && tail.len() >= 10 {
        return Some("a JWT");
    }
    // A Discord bot token: the base64 user id, a time stamp of 6 bytes, and an HMAC.
    let id_start = head.first().is_some_and(|c| matches!(c, b'M' | b'N' | b'O'));
    let discord = id_start && (24..=28).contains(&head.len()) && body.len() == 6 && (27..=40).contains(&tail.len());
    discord.then_some("a Discord bot token")
}

fn segment(bytes: &[u8], (start, len): (usize, usize)) -> &[u8] {
    &bytes[start..start + len]
}

/// Path parts of a commit URL, for example `https://github.com/owner/repo/blob/<commit>/src/lib.rs`.
const URL_PARTS: [&str; 6] = ["/commit/", "/commits/", "/blob/", "/tree/", "/raw/", "/archive/"];

/// Words that name a full commit before it: commit `<commit>`, `rev = "<commit>"`.
const COMMIT_WORDS: [&str; 2] = ["commit", "rev"];

/// The names of a digest. A digest is public by design.
const DIGEST_NAMES: [&str; 4] = ["sha256", "sha-256", "sha512", "sha-512"];

/// A run of 40 or more hex digits. A credential can have this shape, and so
/// can a public commit or a digest. So the run is a finding unless its shape
/// says that it is public: a citation `owner/repo@<commit>`, a commit URL, a
/// line that names a digest, a row of the pins table, or a full commit after
/// the word commit or rev.
fn long_hex(line: &str, kind: FileKind, hits: &mut Vec<Hit>) {
    let bytes = line.as_bytes();
    let mut pos = 0;
    while pos < bytes.len() {
        let len = run(bytes, pos, hex);
        if len >= HEX_MIN && !public_hex(line, pos, len, kind) {
            hits.push(Hit { rule: "a long hex run", at: pos });
        }
        pos += len.max(1);
    }
}

/// True when the shape around a hex run says that the run is a public value.
fn public_hex(line: &str, at: usize, len: usize, kind: FileKind) -> bool {
    // The run starts with an ASCII digit, so `at` is a character boundary.
    let before = &line[..at];
    let lower = line.to_ascii_lowercase();
    before.ends_with('@')
        || URL_PARTS.iter().any(|part| before.ends_with(part))
        || DIGEST_NAMES.iter().any(|name| lower.contains(name))
        || (kind.markdown && is_pins_row(line))
        || (matches!(len, 40 | 64) && named_commit(before))
}

/// True when the text before a run ends with the word commit or rev, and
/// after it only separators: spaces, a backtick, a quote, `=` or `:`.
fn named_commit(before: &str) -> bool {
    let head = before.trim_end_matches([' ', '\t', '`', '"', '\'', '=', ':']);
    if head.len() == before.len() {
        return false;
    }
    let lower = head.to_ascii_lowercase();
    COMMIT_WORDS.iter().any(|w| lower.strip_suffix(w).is_some_and(|rest| !rest.bytes().next_back().is_some_and(word)))
}

/// True for a Markdown table row whose first cell starts with an `owner/repo`
/// in backticks: a row of the pins table of docs/references.md.
fn is_pins_row(line: &str) -> bool {
    let Some(rest) = line.trim_start().strip_prefix('|') else { return false };
    let cell = rest.split('|').next().unwrap_or_default().trim();
    let Some(inner) = cell.strip_prefix('`') else { return false };
    inner.split_once('`').is_some_and(|(name, _)| is_owner_repo(name))
}

fn is_owner_repo(name: &str) -> bool {
    let part =
        |p: &str| !p.is_empty() && p.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'-'));
    name.split_once('/').is_some_and(|(owner, repo)| part(owner) && part(repo))
}

fn alnum(c: u8) -> bool {
    c.is_ascii_alphanumeric()
}

fn upper_alnum(c: u8) -> bool {
    c.is_ascii_uppercase() || c.is_ascii_digit()
}

fn word(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

fn alnum_dash(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'-'
}

fn base64url(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'-' || c == b'_'
}

fn hex(c: u8) -> bool {
    c.is_ascii_hexdigit()
}

/// The length of the run of bytes from `from` that `ok` accepts.
fn run(bytes: &[u8], from: usize, ok: fn(u8) -> bool) -> usize {
    bytes.get(from..).map_or(0, |rest| rest.iter().take_while(|&&c| ok(c)).count())
}

/// True when a token can start at `at`: the byte before it is not a letter or a digit.
fn token_start(bytes: &[u8], at: usize) -> bool {
    at == 0 || !alnum(bytes[at - 1])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A body of `len` letters and digits. No real value is in the source.
    fn body(len: usize) -> String {
        "aB3dE5gH7jK9mN1pQ2sT4vW6yZ8".chars().cycle().take(len).collect()
    }

    /// A run of `len` lowercase hex digits.
    fn hex_run(len: usize) -> String {
        "0123456789abcdef".chars().cycle().take(len).collect()
    }

    /// No rule matched.
    const NONE: [&str; 0] = [];

    fn rules_of(line: &str, kind: FileKind) -> Vec<&'static str> {
        scan_line(line, kind).into_iter().map(|h| h.rule).collect()
    }

    fn rules_in(line: &str) -> Vec<&'static str> {
        rules_of(line, FileKind::default())
    }

    fn rules_in_markdown(line: &str) -> Vec<&'static str> {
        rules_of(line, FileKind::of("docs/references.md"))
    }

    #[test]
    fn finds_a_private_key_header() {
        assert_eq!(rules_in(&format!("-----{KEY_OPEN}RSA {KEY_TAIL}-----")), ["a private key header"]);
        assert_eq!(rules_in(&format!("{KEY_OPEN}{KEY_TAIL}")), ["a private key header"]);
        assert_eq!(rules_in(&format!("{KEY_OPEN}PUBLIC KEY")), NONE);
    }

    #[test]
    fn finds_an_aws_key_id_of_the_exact_length() {
        let id = |tail: &str| format!("key = {}{tail}", ["AK", "IA"].concat());
        assert_eq!(rules_in(&id("ABCDEFGH23456789")), ["an AWS access key id"]);
        assert_eq!(rules_in(&id("ABCDEFGH2345678")), NONE);
        assert_eq!(rules_in(&id("ABCDEFGH23456789x")), NONE);
    }

    #[test]
    fn finds_github_tokens() {
        let classic = ["gh", "p_"].concat();
        assert_eq!(rules_in(&format!("{classic}{}", body(36))), ["a GitHub token"]);
        assert_eq!(rules_in(&format!("{classic}{}", body(35))), NONE);
        let fine = format!("{}{}_{}", ["github", "_pat_"].concat(), body(22), body(59));
        assert_eq!(rules_in(&fine), ["a GitHub token"]);
    }

    #[test]
    fn finds_vendor_tokens_with_a_prefix() {
        let cases = [
            (format!("{}123456789012-{}", ["xo", "xb-"].concat(), body(24)), "a Slack token"),
            (format!("{}{}", ["AI", "za"].concat(), body(35)), "a Google API key"),
            (format!("{}{}", ["sk-", "ant-"].concat(), body(40)), "an sk-ant- API key"),
            (format!("{}{}", ["s", "k-"].concat(), body(40)), "an sk- API key"),
            (format!("{}{}", ["sk_", "live_"].concat(), body(24)), "a Stripe secret key"),
            (format!("{}{}", ["np", "m_"].concat(), body(36)), "an npm token"),
        ];
        for (line, rule) in cases {
            assert_eq!(rules_in(&line), [rule], "{rule}");
        }
    }

    #[test]
    fn a_word_that_ends_in_sk_is_not_a_key() {
        assert_eq!(rules_in("see the risk-assessment-of-the-broker-flow-in-version-2026"), NONE);
        assert_eq!(rules_in(&format!("x{}{}", ["s", "k-"].concat(), body(40))), NONE);
    }

    #[test]
    fn finds_a_jwt_and_a_discord_token() {
        let head = ["ey", "J"].concat();
        let jwt = format!("Bearer {head}{}.{head}{}.{}", body(20), body(30), body(43));
        assert_eq!(rules_in(&jwt), ["a JWT"]);
        let discord = format!("token: M{}.{}.{}", body(23), body(6), body(30));
        assert_eq!(rules_in(&discord), ["a Discord bot token"]);
        assert_eq!(rules_in("see docs/limits.md and the Cargo.toml file"), NONE);
    }

    #[test]
    fn a_long_hex_run_is_a_finding() {
        assert_eq!(rules_in(&format!("token = {}", hex_run(40))), ["a long hex run"]);
        assert_eq!(rules_in(&hex_run(40).to_ascii_uppercase()), ["a long hex run"]);
        assert_eq!(rules_in(&hex_run(39)), NONE);
    }

    #[test]
    fn a_public_commit_or_digest_is_not_a_finding() {
        let commit = hex_run(40);
        let digest = hex_run(64);
        for line in [
            format!("`owner/repo@{commit}:src/lib.rs:10`"),
            format!("https://github.com/owner/repo/blob/{commit}/src/lib.rs#L10"),
            format!("sha256:{digest}"),
            format!("sha256 = \"{digest}\""),
            format!("the image digest (SHA256) is {digest}"),
            format!("both clones resolve to commit `{commit}` on each host"),
            format!("rev = \"{commit}\""),
        ] {
            assert_eq!(rules_in(&line), NONE, "{line}");
        }
    }

    #[test]
    fn the_commit_word_covers_a_full_commit_only() {
        assert_eq!(rules_in(&format!("commit `{}`", hex_run(50))), ["a long hex run"]);
        assert_eq!(rules_in(&format!("recommit `{}`", hex_run(40))), ["a long hex run"]);
    }

    #[test]
    fn the_pins_table_is_public_in_markdown_only() {
        let row = format!("| `owner/repo` | `{}` | 2026-10-14 | the reason |", hex_run(40));
        assert_eq!(rules_in_markdown(&row), NONE);
        assert_eq!(rules_in(&row), ["a long hex run"]);
        let not_a_pin = format!("| owner | `{}` |", hex_run(40));
        assert_eq!(rules_in_markdown(&not_a_pin), ["a long hex run"]);
    }

    #[test]
    fn cargo_lock_has_no_long_hex_rule() {
        let line = format!("checksum = \"{}\"", hex_run(64));
        assert_eq!(rules_of(&line, FileKind::of("Cargo.lock")), NONE);
        assert_eq!(rules_in(&line), ["a long hex run"]);
    }

    #[test]
    fn a_finding_shows_four_characters_only() {
        let token = format!("{}{}", ["gh", "p_"].concat(), body(36));
        let found = scan_text("a.md", &format!("line one\r\nkey: {token}\r\n"));
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].line, 2);
        assert!(found[0].message.ends_with(&format!("{}...", &token[..4])));
        assert!(!found[0].message.contains(&token[4..]));
    }

    #[test]
    fn a_credential_file_is_found_by_its_name() {
        for rel in
            [".env", "app/.env.local", "certs/server.pem", "id_ed25519", "ci/service-account-1.json", ".npm_token"]
        {
            assert!(credential_file(rel).is_some(), "{rel}");
        }
        for rel in [".env.example", "docs/limits.md", "Cargo.toml", "keys.md", "service.json"] {
            assert!(credential_file(rel).is_none(), "{rel}");
        }
    }
}
