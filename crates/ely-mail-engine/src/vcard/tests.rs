use super::{Card, read, write};

fn card(name: Option<&str>, emails: &[&str]) -> Card {
    Card { name: name.map(str::to_owned), emails: emails.iter().map(|each| each.to_string()).collect() }
}

#[test]
fn version_three_and_four_cards_give_names_and_addresses() {
    let text = "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:Smith\\, Ann\r\nN:Smith;Ann;;;\r\n\
                item1.EMAIL;TYPE=\"INTERNET,WORK\":ann@example.com\r\nEMAIL;TYPE=HOME:\r\n \
                ann@home.example\r\nTEL:+1\r\nEND:VCARD\r\n\r\nbegin:vcard\nversion:4.0\n\
                N:Lee;Bo;Q;Dr.;Jr.\nemail:bo@example.com\nend:vcard\n\
                BEGIN:VCARD\nVERSION:3.0\nFN:No Mail\nEND:VCARD\n";
    assert_eq!(
        read(text).unwrap(),
        [
            card(Some("Smith, Ann"), &["ann@example.com", "ann@home.example"]),
            card(Some("Dr. Bo Q Lee Jr."), &["bo@example.com"]),
            card(Some("No Mail"), &[]),
        ]
    );
}

#[test]
fn version_two_one_quoted_printable_is_decoded() {
    // "张三" in UTF-8, broken by a soft line break.
    let text = "BEGIN:VCARD\r\nVERSION:2.1\r\nFN;CHARSET=UTF-8;ENCODING=QUOTED-PRINTABLE:=E5=BC=\r\n\
                =A0=E4=B8=89\r\nN;QUOTED-PRINTABLE:Li;S=69;;;\r\nEMAIL;INTERNET:zhang@example.com\r\nEND:VCARD\r\n";
    assert_eq!(read(text).unwrap(), [card(Some("张三"), &["zhang@example.com"])]);
}

#[test]
fn an_escaped_semicolon_stays_in_its_part() {
    let text = "BEGIN:VCARD\nN:Ross\\;Ward;Ann\nEMAIL:a@example.com\nEND:VCARD\n";
    assert_eq!(read(text).unwrap(), [card(Some("Ann Ross;Ward"), &["a@example.com"])]);
}

#[test]
fn a_malformed_file_names_the_line() {
    let said = |text: &str| read(text).unwrap_err().to_string();
    assert!(said("BEGIN:VCARD\nFN:A\n").contains("no END"));
    assert!(said("FN:A\n").contains("line 1"));
    assert!(said("BEGIN:VCARD\nFN A\nEND:VCARD\n").contains("line 2: no colon"));
    assert!(said("BEGIN:VCARD\nBEGIN:VCARD\n").contains("line 2"));
    let gb = "BEGIN:VCARD\nFN;CHARSET=GB2312;ENCODING=QUOTED-PRINTABLE:=D5=C5\nEND:VCARD\n";
    assert!(said(gb).contains("GB2312"));
    let photo = "BEGIN:VCARD\nFN;ENCODING=b:QQ==\nEND:VCARD\n";
    assert!(said(photo).contains("line 2: the B encoding"));
    let wrong_end = "BEGIN:VCARD\nFN:A\nEMAIL:a@example.com\nEND:OTHER\nEND:VCARD\n";
    assert!(said(wrong_end).contains("line 4: an END of OTHER"), "{}", said(wrong_end));
}

#[test]
fn only_the_four_escapes_read() {
    let said = |text: &str| read(text).unwrap_err().to_string();
    let card = |line: &str| format!("BEGIN:VCARD\nVERSION:4.0\n{line}\nEND:VCARD\n");
    let email = card(r"EMAIL:ann\q@example.com");
    assert!(said(&email).contains(r"line 3: an escape \q"), "{}", said(&email));
    let name = card(r"FN:New\qName");
    assert!(said(&name).contains("line 3"), "{}", said(&name));
    let structured = card(r"N:Ross\x;Ann");
    assert!(said(&structured).contains("line 3"), "{}", said(&structured));
    let sixth = card(r"N:Ross;Ann;;;;bad\q");
    assert!(said(&sixth).contains("line 3: N has 6 parts"), "{}", said(&sixth));
    let trailing = card(r"FN:Ann\");
    assert!(said(&trailing).contains("line 3: a lone backslash"), "{}", said(&trailing));
    let fine = card(r"FN:A\, B\; C\\D\nE\NF");
    assert_eq!(read(&fine).unwrap()[0].name.as_deref(), Some(r"A, B; C\D E F"));
}

#[test]
fn version_two_one_allows_spaces_around_card_bounds() {
    let text = "BEGIN : VCARD \r\nVERSION:2.1\r\nFN:Ann\r\nEMAIL;INTERNET:ann@example.com\r\nEND : VCARD\r\n";
    assert_eq!(read(text).unwrap(), [card(Some("Ann"), &["ann@example.com"])]);
}

#[test]
fn written_cards_fold_and_read_back() {
    let long = "张".repeat(40);
    let people = [
        (Some("Smith, Ann; Jr.\\"), "ann@example.com"),
        (None, "bo@example.com"),
        (Some(long.as_str()), "z@example.com"),
    ];
    let text = write(people);
    assert!(text.split("\r\n").all(|line| line.len() <= 75), "{text}");
    assert!(text.starts_with("BEGIN:VCARD\r\nVERSION:3.0\r\nFN:Smith\\, Ann\\; Jr.\\\\\r\n"));
    assert_eq!(
        read(&text).unwrap(),
        [
            card(Some("Smith, Ann; Jr.\\"), &["ann@example.com"]),
            card(Some("bo@example.com"), &["bo@example.com"]),
            card(Some(&long), &["z@example.com"]),
        ]
    );
}
