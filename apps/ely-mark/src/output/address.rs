//! Addresses, read as browsers read them.

/// A plain, web or mail address, read as browsers do.
pub(super) fn safe(url: &str) -> bool {
    // Browsers trim spaces and controls, then drop tabs and newlines.
    let read: String = url
        .trim_matches(|ch: char| ch <= ' ')
        .chars()
        .filter(|ch| !matches!(ch, '\t' | '\n' | '\r'))
        .collect();
    match scheme(&read) {
        None => true,
        Some(scheme) => ["http", "https", "mailto"].contains(&scheme.to_lowercase().as_str()),
    }
}

/// An address's scheme, when it has one.
pub(super) fn scheme(url: &str) -> Option<&str> {
    let end = url.find(':')?;
    let name = &url[..end];
    let plain = name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || "+-.".contains(ch));
    // A drive letter is a path, not a scheme.
    (plain && name.len() > 1).then_some(name)
}
