use super::{Block, Inline, NESTED};

/// Paragraphs by blank lines; `>` lines quote.
pub(super) fn blocks(text: &str) -> Vec<Block> {
    let lines: Vec<&str> = text
        .split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
        .collect();
    under(&lines, 0)
}

fn under(lines: &[&str], nested: usize) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut at = 0;
    while at < lines.len() {
        let quoted = |line: &str| nested < NESTED && line.starts_with('>');
        if quoted(lines[at]) {
            let end = at + lines[at..].iter().take_while(|line| quoted(line)).count();
            let inner: Vec<&str> = lines[at..end].iter().map(|line| unquoted(line)).collect();
            let inner = under(&inner, nested + 1);
            if !inner.is_empty() {
                blocks.push(Block::Quote(inner));
            }
            at = end;
            continue;
        }
        if lines[at].trim().is_empty() {
            at += 1;
            continue;
        }
        let end = at
            + lines[at..]
                .iter()
                .take_while(|line| !line.trim().is_empty() && !quoted(line))
                .count();
        blocks.push(Block::Text(linked(&lines[at..end].join("\n"))));
        at = end;
    }
    blocks
}

/// RFC 3676 §4.5: one `>`, then its space.
fn unquoted(line: &str) -> &str {
    let rest = &line[1..];
    rest.strip_prefix(' ').unwrap_or(rest)
}

/// Web addresses in plain text become links.
fn linked(text: &str) -> Inline {
    let mut inline = Inline {
        text: text.to_owned(),
        ..Inline::default()
    };
    let lower = text.to_ascii_lowercase();
    let mut from = 0;
    while let Some(found) = ["http://", "https://"]
        .iter()
        .filter_map(|scheme| lower[from..].find(scheme).map(|at| from + at))
        .min()
    {
        let word = text[..found].chars().next_back();
        let rest = &text[found..];
        let len = rest
            .find(|ch: char| ch.is_whitespace() || matches!(ch, '<' | '>' | '"'))
            .unwrap_or(rest.len());
        let url = rest[..len].trim_end_matches(['.', ',', ';', ':', '!', '?', ')', ']', '\'']);
        let at_word = word.is_none_or(|ch| !ch.is_alphanumeric());
        let host = url.split_once("://").map_or("", |(_, host)| host);
        if at_word && !host.is_empty() {
            inline
                .links
                .push((found..found + url.len(), url.to_owned()));
        }
        from = found + len.max(1);
    }
    inline
}
