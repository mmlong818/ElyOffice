//! vCard names and addresses: 2.1 to 4.0 read, 3.0 written.

use crate::{Error, Result};

/// One card's name and addresses.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Card {
    pub name: Option<String>,
    pub emails: Vec<String>,
}

/// One content line: its property, parameters and value.
struct Line {
    number: usize,
    property: String,
    params: Vec<(String, String)>,
    value: String,
}

/// Every card in `text`; a bad line fails it all.
pub fn read(text: &str) -> Result<Vec<Card>> {
    let bad = |number: usize, what: &str| Error::Invalid(format!("vCard line {number}: {what}"));
    let mut cards = Vec::new();
    let mut card: Option<(Card, Option<String>)> = None;
    for line in lines(text)? {
        let property = line.property.as_str();
        // 2.1 allows spaces around bounds: `BEGIN : VCARD`.
        let bound = line.value.trim();
        match (property, card.as_mut()) {
            ("BEGIN", None) if bound.eq_ignore_ascii_case("VCARD") => {
                card = Some((Card { name: None, emails: Vec::new() }, None));
            }
            ("BEGIN", Some(_)) => return Err(bad(line.number, "a card inside a card")),
            ("END", Some(_)) if bound.eq_ignore_ascii_case("VCARD") => {
                let (mut done, structured) = card.take().expect("matched a card");
                // FN is required; N stands in.
                done.name = done.name.or(structured);
                cards.push(done);
            }
            ("END", Some(_)) => return Err(bad(line.number, &format!("an END of {bound}"))),
            (_, None) => return Err(bad(line.number, "outside any card")),
            ("FN", Some((card, _))) => {
                card.name = named(unescape(&decoded(&line)?).map_err(|what| bad(line.number, &what))?)
            }
            ("N", Some((_, structured))) => {
                *structured = named(person(&decoded(&line)?).map_err(|what| bad(line.number, &what))?)
            }
            ("EMAIL", Some((card, _))) => {
                let email = unescape(&decoded(&line)?).map_err(|what| bad(line.number, &what))?;
                let email = email.trim().to_owned();
                if !email.is_empty() {
                    card.emails.push(email);
                }
            }
            _ => {}
        }
    }
    match card {
        Some(_) => Err(Error::Invalid("vCard: the last card has no END".into())),
        None => Ok(cards),
    }
}

/// Cards of `(name, address)`, as vCard 3.0.
pub fn write<'a>(people: impl IntoIterator<Item = (Option<&'a str>, &'a str)>) -> String {
    let mut out = String::new();
    for (name, address) in people {
        // FN is required: the address stands in.
        let shown = name.unwrap_or(address);
        for line in [
            "BEGIN:VCARD".to_owned(),
            "VERSION:3.0".to_owned(),
            format!("FN:{}", escape(shown)),
            "N:;;;;".to_owned(),
            format!("EMAIL;TYPE=INTERNET:{}", escape(address)),
            "END:VCARD".to_owned(),
        ] {
            fold(&line, &mut out);
        }
    }
    out
}

/// Unfolded content lines, blank ones dropped.
fn lines(text: &str) -> Result<Vec<Line>> {
    let mut raw: Vec<(usize, String)> = Vec::new();
    for (index, physical) in text.split('\n').enumerate() {
        let physical = physical.strip_suffix('\r').unwrap_or(physical);
        let continues = raw.last().is_some_and(|(_, last)| soft_break(last));
        match (physical.strip_prefix([' ', '\t']), raw.last_mut()) {
            // 2.1 quoted-printable: a trailing `=` continues.
            (_, Some((_, last))) if continues => {
                last.pop();
                last.push_str(physical);
            }
            // RFC 6350 §3.2: a leading space continues.
            (Some(rest), Some((_, last))) => last.push_str(rest),
            _ if physical.trim().is_empty() => {}
            _ => raw.push((index + 1, physical.to_owned())),
        }
    }
    raw.into_iter().map(|(number, text)| parse(number, &text)).collect()
}

fn soft_break(line: &str) -> bool {
    let head = line.split(':').next().expect("split yields one part");
    line.ends_with('=') && head.to_ascii_uppercase().contains("QUOTED-PRINTABLE")
}

/// `[group.]NAME *(;param) : value`; quotes may hold `:` and `;`.
fn parse(number: usize, text: &str) -> Result<Line> {
    let mut quoted = false;
    let colon = text.char_indices().find_map(|(at, c)| match c {
        '"' => {
            quoted = !quoted;
            None
        }
        ':' if !quoted => Some(at),
        _ => None,
    });
    let Some(colon) = colon else {
        return Err(Error::Invalid(format!("vCard line {number}: no colon")));
    };
    let (head, value) = (&text[..colon], &text[colon + 1..]);
    let mut parts = split_unquoted(head, ';').into_iter();
    let name = parts.next().expect("split yields one part");
    let property = name.rsplit('.').next().expect("rsplit yields one part");
    let params = parts
        .map(|param| match param.split_once('=') {
            Some((key, value)) => (key.trim().to_ascii_uppercase(), value.trim_matches('"').to_owned()),
            // 2.1 writes a bare `QUOTED-PRINTABLE` or `INTERNET`.
            None => (bare_key(param), param.trim().to_owned()),
        })
        .collect();
    Ok(Line { number, property: property.trim().to_ascii_uppercase(), params, value: value.to_owned() })
}

fn bare_key(param: &str) -> String {
    match param.trim().to_ascii_uppercase().as_str() {
        "QUOTED-PRINTABLE" | "BASE64" | "8BIT" | "7BIT" => "ENCODING".into(),
        _ => "TYPE".into(),
    }
}

fn split_unquoted(text: &str, by: char) -> Vec<&str> {
    let (mut parts, mut start, mut quoted) = (Vec::new(), 0, false);
    for (at, c) in text.char_indices() {
        match c {
            '"' => quoted = !quoted,
            c if c == by && !quoted => {
                parts.push(&text[start..at]);
                start = at + c.len_utf8();
            }
            _ => {}
        }
    }
    parts.push(&text[start..]);
    parts
}

/// The value decoded, still escaped; other charsets fail.
fn decoded(line: &Line) -> Result<String> {
    let bad = |what: String| Error::Invalid(format!("vCard line {}: {what}", line.number));
    let param = |key: &str| line.params.iter().find(|(each, _)| each == key).map(|(_, value)| value);
    if let Some(charset) = param("CHARSET")
        && !["UTF-8", "US-ASCII"].contains(&charset.to_ascii_uppercase().as_str())
    {
        return Err(bad(format!("the {charset} charset")));
    }
    let raw = match param("ENCODING").map(|each| each.to_ascii_uppercase()) {
        None => line.value.clone(),
        Some(encoding) if encoding == "QUOTED-PRINTABLE" => {
            let bytes = quoted_printable(&line.value).ok_or_else(|| bad("bad quoted-printable".into()))?;
            String::from_utf8(bytes).map_err(|_| bad("text that is not UTF-8".into()))?
        }
        Some(encoding) if encoding == "8BIT" || encoding == "7BIT" => line.value.clone(),
        Some(encoding) => return Err(bad(format!("the {encoding} encoding"))),
    };
    Ok(raw)
}

fn quoted_printable(text: &str) -> Option<Vec<u8>> {
    let (bytes, mut out, mut at) = (text.as_bytes(), Vec::new(), 0);
    while at < bytes.len() {
        match bytes[at] {
            b'=' => {
                let hex = std::str::from_utf8(bytes.get(at + 1..at + 3)?).ok()?;
                out.push(u8::from_str_radix(hex, 16).ok()?);
                at += 3;
            }
            byte => {
                out.push(byte);
                at += 1;
            }
        }
    }
    Some(out)
}

/// RFC 6350 §3.4: only `\\`, `\,`, `\;` and `\n`.
fn unescape(text: &str) -> std::result::Result<String, String> {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        match (c, c == '\\') {
            (_, true) => match chars.next() {
                Some('n' | 'N') => out.push('\n'),
                Some(kept @ ('\\' | ',' | ';')) => out.push(kept),
                Some(other) => return Err(format!("an escape \\{other}")),
                None => return Err("a lone backslash".into()),
            },
            (c, false) => out.push(c),
        }
    }
    Ok(out)
}

/// N's prefix, given, additional, family, suffix, in reading order.
fn person(value: &str) -> std::result::Result<String, String> {
    let mut parts = vec![String::new()];
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        let last = parts.last_mut().expect("starts with one");
        match c {
            '\\' => {
                last.push(c);
                last.extend(chars.next());
            }
            ';' => parts.push(String::new()),
            c => last.push(c),
        }
    }
    // RFC 6350 §6.2.2: five parts at most.
    if parts.len() > 5 {
        return Err(format!("N has {} parts", parts.len()));
    }
    let at = |index: usize| parts.get(index).map_or(Ok(String::new()), |part| unescape(part.trim()));
    let order = [at(3)?, at(1)?, at(2)?, at(0)?, at(4)?];
    Ok(order.into_iter().filter(|part| !part.is_empty()).collect::<Vec<_>>().join(" "))
}

fn named(text: String) -> Option<String> {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    (!text.is_empty()).then_some(text)
}

fn escape(text: &str) -> String {
    text.replace('\\', "\\\\").replace(',', "\\,").replace(';', "\\;").replace('\n', "\\n")
}

/// RFC 6350 §3.2: lines of at most 75 octets.
fn fold(line: &str, out: &mut String) {
    let mut width = 0;
    for c in line.chars() {
        if width + c.len_utf8() > 75 {
            out.push_str("\r\n ");
            width = 1;
        }
        out.push(c);
        width += c.len_utf8();
    }
    out.push_str("\r\n");
}

#[cfg(test)]
mod tests;
