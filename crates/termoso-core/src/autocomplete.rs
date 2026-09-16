//! Offline autocomplete over the line the user has typed so far. Pure: this
//! module never talks to the network — sources are the bundled command
//! catalogue (`assets/autocomplete`, shared with the desktop client), the
//! decrypted command history, snippets and directory listings the caller
//! fetches through the session itself.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

const COMMAND_ROWS: &str = include_str!("../assets/autocomplete/commands.txt");
const WRAPPER_ROWS: &str = include_str!("../assets/autocomplete/wrappers.txt");
const OPTION_ROWS: &str = include_str!("../assets/autocomplete/options.txt");
const SUBCOMMAND_ROWS: &str = include_str!("../assets/autocomplete/subcommands.txt");

/// Most items a completion ever returns.
pub const MAX_SUGGESTIONS: usize = 12;
/// Most entries a directory listing is read for.
pub const MAX_DIR_ENTRIES: usize = 2000;
const MAX_HISTORY: usize = 4;
const MAX_SNIPPETS: usize = 3;
const MAX_COMMANDS: usize = 8;

/// Where a suggestion came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuggestionKind {
    /// A command name from the catalogue.
    Command,
    /// A flag of the current command.
    Option,
    /// A subcommand of the current command.
    Subcommand,
    /// A file or directory from a listing.
    Path,
    /// A previously run command line.
    History,
    /// A one-line snippet.
    Snippet,
}

/// One completion candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    /// Source of the candidate.
    pub kind: SuggestionKind,
    /// Full token / line shown to the user.
    pub label: String,
    /// Short description (command help, snippet name, `history`…).
    pub desc: String,
    /// Text to type to turn what is on the line into `label`.
    pub insert: String,
}

/// A snippet as the completion sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnippetSource {
    /// Snippet name.
    pub label: String,
    /// Snippet body; only single-line scripts are ever suggested.
    pub script: String,
}

/// A directory the caller should list to finish the path part of the
/// completion (see [`merge_paths`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathQuery {
    /// Directory as typed (`src/`, `~/`, `/etc/`); empty = working directory.
    pub dir: String,
    /// Unescaped file-name prefix typed after the last slash.
    pub prefix: String,
    /// The command only takes directories (`cd`).
    pub dirs_only: bool,
}

/// Result of [`complete`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Completion {
    /// Candidates, best first.
    pub items: Vec<Suggestion>,
    /// Directory to list for path candidates, if the token may be a path.
    pub path: Option<PathQuery>,
}

/// One entry of a directory listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirEntry {
    /// File name without the directory.
    pub name: String,
    /// Directory (or symlink to one).
    pub dir: bool,
}

/// What kind of file arguments a command takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathKind {
    /// Files and directories.
    Any,
    /// Directories only.
    Dir,
    /// No file arguments.
    None,
}

/// A command from the bundled catalogue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSpec {
    /// Command name.
    pub name: &'static str,
    /// One-line description.
    pub desc: &'static str,
    /// What file arguments it takes.
    pub paths: PathKind,
}

/// An option or subcommand of a command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Flag {
    /// Flag or subcommand as typed.
    pub name: &'static str,
    /// One-line description.
    pub desc: &'static str,
}

struct Catalogue {
    commands: Vec<CommandSpec>,
    by_name: HashMap<&'static str, usize>,
    wrappers: HashSet<&'static str>,
    options: HashMap<&'static str, Vec<Flag>>,
    subcommands: HashMap<&'static str, Vec<Flag>>,
}

/// `flag description;flag description;…`
fn parse_flags(spec: &'static str) -> Vec<Flag> {
    spec.split(';')
        .filter(|item| !item.is_empty())
        .map(|item| match item.find(' ') {
            Some(sp) => Flag {
                name: &item[..sp],
                desc: &item[sp + 1..],
            },
            None => Flag {
                name: item,
                desc: "",
            },
        })
        .collect()
}

/// `command|flag description;flag description;…` per line.
fn parse_table(rows: &'static str) -> HashMap<&'static str, Vec<Flag>> {
    rows.lines()
        .filter_map(|line| line.split_once('|'))
        .filter(|(name, _)| !name.is_empty())
        .map(|(name, spec)| (name, parse_flags(spec)))
        .collect()
}

static CATALOGUE: LazyLock<Catalogue> = LazyLock::new(|| {
    // `name|description[|d|n]` — `d` = takes directories, `n` = takes no paths.
    let commands: Vec<CommandSpec> = COMMAND_ROWS
        .lines()
        .filter(|l| !l.is_empty())
        .filter_map(|line| {
            let mut parts = line.split('|');
            let name = parts.next().filter(|n| !n.is_empty())?;
            let desc = parts.next().unwrap_or("");
            let paths = match parts.next() {
                Some("d") => PathKind::Dir,
                Some("n") => PathKind::None,
                _ => PathKind::Any,
            };
            Some(CommandSpec { name, desc, paths })
        })
        .collect();
    let by_name = commands
        .iter()
        .enumerate()
        .map(|(i, c)| (c.name, i))
        .collect();
    Catalogue {
        commands,
        by_name,
        wrappers: WRAPPER_ROWS.lines().filter(|l| !l.is_empty()).collect(),
        options: parse_table(OPTION_ROWS),
        subcommands: parse_table(SUBCOMMAND_ROWS),
    }
});

/// The bundled command dictionary, in catalogue order.
pub fn commands() -> &'static [CommandSpec] {
    &CATALOGUE.commands
}

/// Catalogue entry of `name`, if known.
pub fn command_spec(name: &str) -> Option<&'static CommandSpec> {
    CATALOGUE.by_name.get(name).map(|&i| &CATALOGUE.commands[i])
}

/// Frequent options of `command` (empty when unknown).
pub fn options_for(command: &str) -> &'static [Flag] {
    CATALOGUE
        .options
        .get(command)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

/// First-level subcommands of `command` (empty when it has none).
pub fn subcommands_for(command: &str) -> &'static [Flag] {
    CATALOGUE
        .subcommands
        .get(command)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

/// Commands that take another command as their argument (`sudo ls`).
pub fn is_wrapper(word: &str) -> bool {
    CATALOGUE.wrappers.contains(word)
}

/// Last simple command of the line (after `|`, `&&`, `;` …).
fn last_segment(line: &str) -> &str {
    let mut start = 0;
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'|' | b'&' => {
                i += 1;
                if i < bytes.len() && bytes[i] == bytes[i - 1] {
                    i += 1;
                }
                start = i;
            }
            b';' => {
                i += 1;
                start = i;
            }
            _ => i += 1,
        }
    }
    &line[start..]
}

/// Whitespace split that keeps quoted / escaped spaces inside a token.
/// Returns the finished words and the token under the cursor.
fn tokenize(segment: &str) -> (Vec<String>, String) {
    let mut words = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut chars = segment.chars().peekable();
    while let Some(ch) = chars.next() {
        if let Some(q) = quote {
            cur.push(ch);
            if ch == q {
                quote = None;
            }
            continue;
        }
        match ch {
            '\\' if chars.peek().is_some() => {
                cur.push(ch);
                if let Some(next) = chars.next() {
                    cur.push(next);
                }
            }
            '\'' | '"' => {
                quote = Some(ch);
                cur.push(ch);
            }
            ' ' | '\t' => {
                if !cur.is_empty() {
                    words.push(std::mem::take(&mut cur));
                }
            }
            _ => cur.push(ch),
        }
    }
    (words, cur)
}

fn is_assignment(word: &str) -> bool {
    let Some((name, _)) = word.split_once('=') else {
        return false;
    };
    let mut chars = name.chars();
    matches!(chars.next(), Some(c) if c == '_' || c.is_ascii_alphabetic())
        && chars.all(|c| c == '_' || c.is_ascii_alphanumeric())
}

/// Index of the real command word, skipping `VAR=x`, `sudo`, `time`… and
/// their flags.
fn command_index(words: &[String]) -> usize {
    let mut i = 0;
    while i < words.len() && is_assignment(&words[i]) {
        i += 1;
    }
    while i < words.len() && is_wrapper(&words[i]) {
        i += 1;
        while i < words.len() && (words[i].starts_with('-') || is_assignment(&words[i])) {
            i += 1;
        }
    }
    i
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some(next) => out.push(next),
                None => out.push(ch),
            }
        } else {
            out.push(ch);
        }
    }
    out
}

/// Shell-escape the characters that would otherwise split or expand a file
/// name.
pub fn escape_path(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        if matches!(
            ch,
            ' ' | '\''
                | '"'
                | '\\'
                | '$'
                | '&'
                | '|'
                | ';'
                | '<'
                | '>'
                | '('
                | ')'
                | '*'
                | '?'
                | '['
                | ']'
                | '#'
                | '~'
                | '!'
                | '{'
                | '}'
                | '`'
        ) {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

fn looks_like_path(token: &str) -> bool {
    token.starts_with('/')
        || token.starts_with('~')
        || token.starts_with('.')
        || token.contains('/')
}

fn path_query(current: &str, dirs_only: bool) -> Option<PathQuery> {
    if current.starts_with('\'')
        || current.starts_with('"')
        || current.starts_with('-')
        || current.starts_with('$')
    {
        return None;
    }
    let (dir, rest) = match current.rfind('/') {
        Some(slash) => (&current[..=slash], &current[slash + 1..]),
        None => ("", current),
    };
    let prefix = unescape(rest);
    if dir.is_empty() && prefix.starts_with('~') {
        return None;
    }
    Some(PathQuery {
        dir: dir.to_string(),
        prefix,
        dirs_only,
    })
}

fn history_items(line: &str, history: &[String]) -> Vec<Suggestion> {
    let mut out = Vec::new();
    let typed = line.trim_start();
    if typed.is_empty() {
        return out;
    }
    for h in history {
        if h.len() > typed.len() && h.starts_with(typed) && !h.contains('\n') {
            out.push(Suggestion {
                kind: SuggestionKind::History,
                label: h.clone(),
                desc: "history".into(),
                insert: h[typed.len()..].to_string(),
            });
            if out.len() >= MAX_HISTORY {
                break;
            }
        }
    }
    out
}

fn snippet_items(current: &str, snippets: &[SnippetSource]) -> Vec<Suggestion> {
    let mut out = Vec::new();
    if current.chars().count() < 2 {
        return out;
    }
    for s in snippets {
        let mut lines = s.script.lines().filter(|l| !l.trim().is_empty());
        let Some(first) = lines.next().map(str::trim) else {
            continue;
        };
        if lines.next().is_some() || !first.starts_with(current) || first == current {
            continue;
        }
        out.push(Suggestion {
            kind: SuggestionKind::Snippet,
            label: first.to_string(),
            desc: s.label.clone(),
            insert: first[current.len()..].to_string(),
        });
        if out.len() >= MAX_SNIPPETS {
            break;
        }
    }
    out
}

struct Collector {
    items: Vec<Suggestion>,
    seen: HashSet<String>,
}

impl Collector {
    fn new(items: Vec<Suggestion>) -> Self {
        let seen = items.iter().map(|i| i.label.clone()).collect();
        Self { items, seen }
    }

    fn push(&mut self, s: Suggestion) {
        if self.seen.insert(s.label.clone()) {
            self.items.push(s);
        }
    }

    fn finish(mut self, path: Option<PathQuery>) -> Completion {
        self.items.truncate(MAX_SUGGESTIONS);
        Completion {
            items: self.items,
            path,
        }
    }
}

fn prefix_items<'a>(
    kind: SuggestionKind,
    current: &str,
    flags: impl Iterator<Item = (&'a str, &'a str)>,
    limit: usize,
) -> Vec<Suggestion> {
    flags
        .filter(|(name, _)| name.starts_with(current) && *name != current)
        .map(|(name, desc)| Suggestion {
            kind,
            label: name.to_string(),
            desc: desc.to_string(),
            insert: name[current.len()..].to_string(),
        })
        .take(limit)
        .collect()
}

/// Suggestions for `line` (input between the prompt and the cursor).
/// `history` holds distinct command lines, most recent first. Everything
/// here resolves synchronously; when the token could be a file name, `path`
/// tells the caller which directory to list (see [`merge_paths`]).
pub fn complete(line: &str, history: &[String], snippets: &[SnippetSource]) -> Completion {
    if line.trim().is_empty() {
        return Completion::default();
    }
    let segment = last_segment(line);
    let (words, current) = tokenize(segment);
    let cmd_idx = command_index(&words);
    let mut out = Collector::new(history_items(line, history));

    if cmd_idx >= words.len() {
        // Typing the command itself.
        if current.is_empty() {
            return out.finish(None);
        }
        let mut path = None;
        if looks_like_path(&current) {
            path = path_query(&current, false);
        } else if !current.starts_with('-') && !current.starts_with('$') {
            let found = prefix_items(
                SuggestionKind::Command,
                &current,
                commands().iter().map(|c| (c.name, c.desc)),
                MAX_COMMANDS,
            );
            for s in found {
                out.push(s);
            }
        }
        for s in snippet_items(segment.trim_start(), snippets) {
            out.push(s);
        }
        return out.finish(path);
    }

    for s in snippet_items(segment.trim_start(), snippets) {
        out.push(s);
    }
    let cmd = words[cmd_idx].as_str();
    let spec = command_spec(cmd);
    let arg_words = &words[cmd_idx + 1..];

    if current.starts_with('-') {
        let found = prefix_items(
            SuggestionKind::Option,
            &current,
            options_for(cmd).iter().map(|f| (f.name, f.desc)),
            usize::MAX,
        );
        for s in found {
            out.push(s);
        }
        return out.finish(None);
    }

    let subs = subcommands_for(cmd);
    let first_arg = arg_words.iter().all(|w| w.starts_with('-'));
    if !subs.is_empty() && first_arg && !looks_like_path(&current) {
        let found = prefix_items(
            SuggestionKind::Subcommand,
            &current,
            subs.iter().map(|f| (f.name, f.desc)),
            usize::MAX,
        );
        for s in found {
            out.push(s);
        }
    }

    let wants_path = looks_like_path(&current)
        || spec.is_none_or(|s| s.paths != PathKind::None)
        || (!subs.is_empty() && !first_arg);
    let mut path = None;
    if wants_path && !(!subs.is_empty() && first_arg && current.is_empty()) {
        path = path_query(&current, spec.is_some_and(|s| s.paths == PathKind::Dir));
    }
    out.finish(path)
}

/// Turn a directory listing into suggestions for `query`, appended after
/// `items` (deduplicated, capped at [`MAX_SUGGESTIONS`]).
pub fn merge_paths(
    items: &[Suggestion],
    query: &PathQuery,
    entries: &[DirEntry],
) -> Vec<Suggestion> {
    let mut out = items.to_vec();
    let mut seen: HashSet<String> = out.iter().map(|i| i.label.clone()).collect();
    let show_hidden = query.prefix.starts_with('.');
    let mut matches: Vec<&DirEntry> = entries
        .iter()
        .filter(|e| !query.dirs_only || e.dir)
        .filter(|e| e.name.starts_with(&query.prefix) && e.name != query.prefix)
        .filter(|e| show_hidden || !e.name.starts_with('.'))
        .collect();
    matches.sort_by(|a, b| b.dir.cmp(&a.dir).then_with(|| a.name.cmp(&b.name)));
    for e in matches {
        if out.len() >= MAX_SUGGESTIONS {
            break;
        }
        let label = format!("{}{}{}", query.dir, e.name, if e.dir { "/" } else { "" });
        if !seen.insert(label.clone()) {
            continue;
        }
        let rest = &e.name[query.prefix.len()..];
        out.push(Suggestion {
            kind: SuggestionKind::Path,
            label,
            desc: if e.dir { "directory" } else { "file" }.into(),
            insert: format!("{}{}", escape_path(rest), if e.dir { "/" } else { " " }),
        });
    }
    out
}

/// What to type after a non-path suggestion is accepted.
pub fn suffix_for(kind: SuggestionKind) -> &'static str {
    match kind {
        SuggestionKind::Command | SuggestionKind::Subcommand | SuggestionKind::Option => " ",
        _ => "",
    }
}

/// Entries of `path` on this machine: relative paths resolve against `cwd`
/// (falling back to the home directory), `~` against home. Unreadable
/// directories yield an empty list.
pub fn list_local_dir(cwd: Option<&str>, path: &str) -> Vec<DirEntry> {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from);
    let expanded: PathBuf = if let Some(rest) = path.strip_prefix('~') {
        let Some(home) = home else {
            return Vec::new();
        };
        home.join(rest.trim_start_matches(['/', '\\']))
    } else if Path::new(path).is_absolute() {
        PathBuf::from(path)
    } else {
        let base = cwd.map(PathBuf::from).or(home).unwrap_or_default();
        base.join(path)
    };
    let Ok(read) = std::fs::read_dir(&expanded) else {
        return Vec::new();
    };
    let mut out: Vec<DirEntry> = read
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let name = e.file_name().to_str()?.to_string();
            let dir = e
                .file_type()
                .map(|t| t.is_dir() || (t.is_symlink() && e.path().is_dir()))
                .unwrap_or(false);
            Some(DirEntry { name, dir })
        })
        .take(MAX_DIR_ENTRIES)
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// One `ls` that works in GNU coreutils, BusyBox and the BSDs: `-1` one per
/// line, `-a` hidden files too, `-p` slash after directories, `-L` so a
/// symlink to a directory gets the slash. Meant for a side `exec` channel;
/// relative paths resolve against `cwd` when known, `~` against `$HOME`.
/// Parse the output with [`parse_ls`].
pub fn remote_ls_command(cwd: Option<&str>, path: &str) -> String {
    let (base, target) = match path.strip_prefix('~') {
        Some(rest) => (
            Some("\"$HOME\"".to_string()),
            format!(".{}", if rest.is_empty() { "/" } else { rest }),
        ),
        None if path.starts_with('/') => (None, path.to_string()),
        None => (cwd.map(shell_quote), path.to_string()),
    };
    let cd = base
        .map(|b| format!("cd {b} 2>/dev/null && "))
        .unwrap_or_default();
    format!(
        "{cd}LC_ALL=C ls -1apL -- {} 2>/dev/null | head -n {}",
        shell_quote(&target),
        MAX_DIR_ENTRIES + 2
    )
}

/// Output of [`remote_ls_command`] as entries (`./` and `../` dropped).
pub fn parse_ls(out: &str) -> Vec<DirEntry> {
    out.lines()
        .filter(|l| !l.is_empty() && *l != "./" && *l != "../")
        .map(|l| match l.strip_suffix('/') {
            Some(name) => DirEntry {
                name: name.to_string(),
                dir: true,
            },
            None => DirEntry {
                name: l.to_string(),
                dir: false,
            },
        })
        .take(MAX_DIR_ENTRIES)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(c: &Completion) -> Vec<&str> {
        c.items.iter().map(|s| s.label.as_str()).collect()
    }

    #[test]
    fn catalogue_loads() {
        assert!(commands().len() > 600);
        assert_eq!(command_spec("cd").unwrap().paths, PathKind::Dir);
        assert_eq!(command_spec("pwd").unwrap().paths, PathKind::None);
        assert_eq!(command_spec("ls").unwrap().paths, PathKind::Any);
        assert!(is_wrapper("sudo"));
        assert!(
            options_for("ls")
                .iter()
                .any(|f| f.name == "-l" && f.desc == "long format")
        );
        assert!(subcommands_for("git").iter().any(|f| f.name == "commit"));
        assert!(options_for("nosuchcmd").is_empty());
    }

    #[test]
    fn commands_by_prefix() {
        let c = complete("gi", &[], &[]);
        assert!(c.items.iter().all(|s| s.kind == SuggestionKind::Command));
        assert!(labels(&c).contains(&"git"));
        assert!(c.items.len() <= MAX_COMMANDS);
        let git = c.items.iter().find(|s| s.label == "git").unwrap();
        assert_eq!(git.insert, "t");
        assert!(c.path.is_none());
        assert_eq!(complete("   ", &[], &[]), Completion::default());
    }

    #[test]
    fn history_first_and_deduplicated() {
        let history = vec![
            "git status".to_string(),
            "git push origin main".to_string(),
            "grep -r foo .".to_string(),
            "multi\nline".to_string(),
        ];
        let c = complete("gi", &history, &[]);
        assert_eq!(c.items[0].kind, SuggestionKind::History);
        assert_eq!(c.items[0].label, "git status");
        assert_eq!(c.items[0].insert, "t status");
        assert_eq!(c.items[1].label, "git push origin main");
        assert!(!labels(&c).contains(&"multi\nline"));
        let seen: HashSet<_> = labels(&c).into_iter().collect();
        assert_eq!(seen.len(), c.items.len());
    }

    #[test]
    fn options_and_subcommands() {
        let c = complete("ls -", &[], &[]);
        assert!(c.items.iter().all(|s| s.kind == SuggestionKind::Option));
        assert!(labels(&c).contains(&"-l"));
        assert!(c.path.is_none());

        let c = complete("git ch", &[], &[]);
        assert!(labels(&c).contains(&"checkout"));
        assert!(labels(&c).contains(&"cherry-pick"));
        let checkout = c.items.iter().find(|s| s.label == "checkout").unwrap();
        assert_eq!(checkout.insert, "eckout");
        assert!(
            c.path.is_none(),
            "first argument of git is a subcommand, not a path"
        );

        let c = complete("git checkout sr", &[], &[]);
        assert!(c.items.is_empty());
        assert_eq!(
            c.path,
            Some(PathQuery {
                dir: "".into(),
                prefix: "sr".into(),
                dirs_only: false
            })
        );
    }

    #[test]
    fn wrappers_and_assignments_are_skipped() {
        let c = complete("sudo -E FOO=1 systemc", &[], &[]);
        assert!(labels(&c).contains(&"systemctl"));
        let c = complete("FOO=1 sudo apt ins", &[], &[]);
        assert!(labels(&c).contains(&"install"));
    }

    #[test]
    fn segments_split_on_operators() {
        let c = complete("cd /tmp && gi", &[], &[]);
        assert!(labels(&c).contains(&"git"));
        let c = complete("ls | gr", &[], &[]);
        assert!(labels(&c).contains(&"grep"));
        let c = complete("a; echo 'x y' ; tai", &[], &[]);
        assert!(labels(&c).contains(&"tail"));
    }

    #[test]
    fn path_queries() {
        let c = complete("cd sr", &[], &[]);
        assert_eq!(
            c.path,
            Some(PathQuery {
                dir: "".into(),
                prefix: "sr".into(),
                dirs_only: true
            })
        );
        let c = complete("cat /etc/pa", &[], &[]);
        assert_eq!(c.path.as_ref().unwrap().dir, "/etc/");
        assert_eq!(c.path.as_ref().unwrap().prefix, "pa");
        let c = complete("cat ~/.s", &[], &[]);
        assert_eq!(c.path.as_ref().unwrap().dir, "~/");
        assert_eq!(c.path.as_ref().unwrap().prefix, ".s");
        assert!(complete("cat ~", &[], &[]).path.is_none());
        assert!(complete("cat \"quoted", &[], &[]).path.is_none());
        assert!(complete("pwd x", &[], &[]).path.is_none());
        assert!(complete("./scr", &[], &[]).path.is_some());
        assert_eq!(
            complete("cat my\\ fi", &[], &[]).path.unwrap().prefix,
            "my fi"
        );
    }

    #[test]
    fn snippets_single_line_only() {
        let snippets = vec![
            SnippetSource {
                label: "Disk usage".into(),
                script: "df -h /".into(),
            },
            SnippetSource {
                label: "Two lines".into(),
                script: "df -h\nfree -m".into(),
            },
        ];
        let c = complete("df", &[], &snippets);
        let s = c
            .items
            .iter()
            .find(|s| s.kind == SuggestionKind::Snippet)
            .unwrap();
        assert_eq!(s.label, "df -h /");
        assert_eq!(s.desc, "Disk usage");
        assert_eq!(s.insert, " -h /");
        assert_eq!(
            c.items
                .iter()
                .filter(|s| s.kind == SuggestionKind::Snippet)
                .count(),
            1
        );
        assert!(
            complete("d", &[], &snippets)
                .items
                .iter()
                .all(|s| s.kind != SuggestionKind::Snippet)
        );
    }

    #[test]
    fn paths_merge_sorted_and_escaped() {
        let query = PathQuery {
            dir: "src/".into(),
            prefix: "m".into(),
            dirs_only: false,
        };
        let entries = vec![
            DirEntry {
                name: "main.rs".into(),
                dir: false,
            },
            DirEntry {
                name: "mod".into(),
                dir: true,
            },
            DirEntry {
                name: "my file".into(),
                dir: false,
            },
            DirEntry {
                name: ".mhidden".into(),
                dir: false,
            },
            DirEntry {
                name: "other".into(),
                dir: false,
            },
        ];
        let out = merge_paths(&[], &query, &entries);
        let labels: Vec<&str> = out.iter().map(|s| s.label.as_str()).collect();
        assert_eq!(labels, ["src/mod/", "src/main.rs", "src/my file"]);
        assert_eq!(out[0].insert, "od/");
        assert_eq!(out[1].insert, "ain.rs ");
        assert_eq!(out[2].insert, "y\\ file ");

        let dirs = merge_paths(
            &[],
            &PathQuery {
                dirs_only: true,
                ..query.clone()
            },
            &entries,
        );
        assert_eq!(dirs.len(), 1);

        let hidden = merge_paths(
            &[],
            &PathQuery {
                prefix: ".".into(),
                ..query
            },
            &entries,
        );
        assert_eq!(hidden.len(), 1);
        assert_eq!(hidden[0].label, "src/.mhidden");
    }

    #[test]
    fn suffixes() {
        assert_eq!(suffix_for(SuggestionKind::Command), " ");
        assert_eq!(suffix_for(SuggestionKind::History), "");
        assert_eq!(suffix_for(SuggestionKind::Path), "");
    }

    #[test]
    fn remote_listing_command_and_parsing() {
        let home = remote_ls_command(None, "~/pro");
        assert!(home.starts_with("cd \"$HOME\" 2>/dev/null && "));
        assert!(home.contains("ls -1apL -- './pro'"));
        let abs = remote_ls_command(Some("/work"), "/etc/");
        assert!(!abs.contains("cd "));
        assert!(abs.contains("-- '/etc/'"));
        let rel = remote_ls_command(Some("/it's"), "src/");
        assert!(rel.contains("cd '/it'\\''s' 2>/dev/null && "));
        assert!(remote_ls_command(None, "src/").starts_with("LC_ALL=C ls"));

        let entries = parse_ls("./\n../\nbin/\nREADME.md\n\n.git/\n");
        assert_eq!(
            entries,
            vec![
                DirEntry {
                    name: "bin".into(),
                    dir: true
                },
                DirEntry {
                    name: "README.md".into(),
                    dir: false
                },
                DirEntry {
                    name: ".git".into(),
                    dir: true
                },
            ]
        );
    }

    #[test]
    fn local_listing() {
        let tmp = std::env::temp_dir().join(format!("termoso-ac-{}", std::process::id()));
        std::fs::create_dir_all(tmp.join("sub")).unwrap();
        std::fs::write(tmp.join("file.txt"), b"x").unwrap();
        let entries = list_local_dir(None, tmp.to_str().unwrap());
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().any(|e| e.name == "sub" && e.dir));
        assert!(entries.iter().any(|e| e.name == "file.txt" && !e.dir));
        let rel = list_local_dir(tmp.to_str(), "sub/");
        assert!(rel.is_empty());
        assert!(list_local_dir(None, "/definitely/not/here").is_empty());
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
