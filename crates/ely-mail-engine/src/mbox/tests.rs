use super::{Reader, write};

fn all(text: &[u8]) -> std::io::Result<Vec<Vec<u8>>> {
    let mut reader = Reader::new(text);
    let mut messages = Vec::new();
    while let Some(message) = reader.next_message()? {
        messages.push(message);
    }
    Ok(messages)
}

#[test]
fn messages_split_on_from_lines_and_unquote() {
    let text = b"From ann@example.com Tue Jan  2 10:00:00 2024\n\
                 Subject: one\n\nhi\n>From the start\n>>From deep\nFrom-less line\n\n\
                 From bo@example.com Wed Jan  3 10:00:00 2024\r\n\
                 Subject: two\r\n\r\nbye\r\n";
    assert_eq!(
        all(text).unwrap(),
        [
            b"Subject: one\r\n\r\nhi\r\nFrom the start\r\n>From deep\r\nFrom-less line\r\n".to_vec(),
            b"Subject: two\r\n\r\nbye\r\n".to_vec(),
        ]
    );
}

#[test]
fn an_empty_file_has_none_and_text_alone_is_refused() {
    assert!(all(b"").unwrap().is_empty());
    assert!(all(b"\n\n").unwrap().is_empty());
    assert!(all(b"Subject: loose\n\nhi\n").is_err());
    assert!(all(b"Subject: loose\nFrom a@x Tue Jan  2 10:00:00 2024\nSubject: x\n").is_err());
}

#[test]
fn written_messages_read_back_whole() {
    let messages: [&[u8]; 2] =
        [b"Subject: one\r\n\r\nFrom here\r\n>From there\r\n", b"Subject: two\r\n\r\nend\r\n"];
    let mut out = Vec::new();
    write(&mut out, "ann@example.com", 1_704_189_600, messages[0]).unwrap();
    write(&mut out, "", 1_704_189_600, messages[1]).unwrap();
    let text = String::from_utf8(out.clone()).unwrap();
    assert!(text.starts_with("From ann@example.com Tue Jan  2 10:00:00 2024\n"), "{text}");
    assert!(text.contains("\n>From here\n>>From there\n"), "{text}");
    assert!(text.contains("\nFrom MAILER-DAEMON Tue Jan  2 10:00:00 2024\n"), "{text}");
    assert_eq!(all(&out).unwrap(), messages.map(<[u8]>::to_vec));
}
