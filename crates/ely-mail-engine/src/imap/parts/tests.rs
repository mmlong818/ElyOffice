use std::collections::HashMap;

use imap_proto::{AttributeValue, Response};

use super::Node;

/// The node of a `* 1 FETCH (BODYSTRUCTURE …)` line.
fn node(structure: &str) -> Node {
    let line = format!("* 1 FETCH (BODYSTRUCTURE {structure})\r\n");
    let (_, response) = Response::parse(line.as_bytes()).unwrap();
    let Response::Fetch(_, attributes) = response else {
        panic!("not a FETCH: {line}");
    };
    let [AttributeValue::BodyStructure(structure)] = attributes.as_slice() else {
        panic!("no structure: {line}");
    };
    Node::read(structure).unwrap()
}

/// Reads the rebuilt message; sections not given come empty.
fn rebuilt(node: &Node, given: &[(&str, &str)]) -> crate::Mail {
    let mut got: HashMap<String, Vec<u8>> = given
        .iter()
        .map(|(key, text)| ((*key).to_owned(), text.replace('\n', "\r\n").into_bytes()))
        .collect();
    for section in node.sections() {
        got.entry(section).or_default();
    }
    crate::read(&node.build(&got).unwrap()).unwrap()
}

const TEXT: &str = "(\"TEXT\" \"PLAIN\" (\"CHARSET\" \"us-ascii\") NIL NIL \"7BIT\" 6 1)";
const HTML: &str = "(\"TEXT\" \"HTML\" (\"CHARSET\" \"us-ascii\") NIL NIL \"7BIT\" 9 1)";

#[test]
fn an_attached_multipart_waits_whole() {
    let structure = format!(
        "({TEXT}({TEXT}{HTML} \"ALTERNATIVE\" (\"BOUNDARY\" \"b2\") \
         (\"ATTACHMENT\" (\"FILENAME\" \"note\")) NIL NIL) \"MIXED\" (\"BOUNDARY\" \"b1\") NIL NIL NIL)"
    );
    let node = node(&structure);
    assert!(!node.complete());
    assert_eq!(node.missing(), ["2"]);
    assert_eq!(node.sections(), ["HEADER", "1.MIME", "1", "2.MIME"]);
    let mail = rebuilt(
        &node,
        &[
            (
                "HEADER",
                "MIME-Version: 1.0\nContent-Type: multipart/mixed; boundary=\"b1\"\n\n",
            ),
            ("1.MIME", "Content-Type: text/plain\n\n"),
            ("1", "hello\n"),
            (
                "2.MIME",
                "Content-Type: multipart/alternative; boundary=\"b2\"\n\
                 Content-Disposition: attachment; filename=\"note\"\n\n",
            ),
        ],
    );
    assert_eq!(mail.text.as_deref().map(str::trim_end), Some("hello"));
    assert_eq!(mail.attachments.len(), 1);
    assert_eq!(mail.attachments[0].content_type, "multipart/alternative");
}

#[test]
fn a_related_image_marked_attachment_comes_down() {
    let structure = format!(
        "({HTML}(\"IMAGE\" \"PNG\" NIL \"<logo>\" NIL \"BASE64\" 12 NIL \
         (\"ATTACHMENT\" (\"FILENAME\" \"logo.png\")) NIL NIL) \
         \"RELATED\" (\"BOUNDARY\" \"r1\") NIL NIL NIL)"
    );
    let node = node(&structure);
    assert!(node.complete());
    assert!(node.missing().is_empty());
    let mail = rebuilt(
        &node,
        &[
            (
                "HEADER",
                "MIME-Version: 1.0\nContent-Type: multipart/related; boundary=\"r1\"\n\n",
            ),
            ("1.MIME", "Content-Type: text/html\n\n"),
            ("1", "<img src=\"cid:logo\">\n"),
            (
                "2.MIME",
                "Content-Type: image/png\nContent-ID: <logo>\n\
                 Content-Disposition: attachment; filename=\"logo.png\"\n\
                 Content-Transfer-Encoding: base64\n\n",
            ),
            ("2", "iVBORw0KGgo=\n"),
        ],
    );
    assert_eq!(mail.inline.len(), 1);
    assert_eq!(mail.inline[0].bytes, b"\x89PNG\r\n\x1a\n");
}

#[test]
fn a_related_start_root_ignores_its_disposition() {
    let structure = "((\"IMAGE\" \"PNG\" NIL NIL NIL \"BASE64\" 8 NIL NIL NIL NIL)\
         (\"TEXT\" \"HTML\" NIL \"<root>\" NIL \"7BIT\" 9 1 NIL (\"ATTACHMENT\" NIL) NIL NIL) \
         \"RELATED\" (\"BOUNDARY\" \"r1\" \"START\" \"<root>\") NIL NIL NIL)";
    let node = node(structure);
    assert_eq!(node.missing(), ["1"]);
}

#[test]
fn an_attached_message_root_waits_whole() {
    let structure =
        format!("({TEXT}{TEXT} \"MIXED\" (\"BOUNDARY\" \"b1\") (\"ATTACHMENT\" NIL) NIL NIL)");
    let node = node(&structure);
    assert_eq!(node.missing(), ["TEXT"]);
    assert_eq!(node.sections(), ["HEADER"]);
    let head = "Content-Type: multipart/mixed; boundary=\"b1\"\nContent-Disposition: attachment\n\n";
    let mail = rebuilt(&node, &[("HEADER", head)]);
    let sections: Vec<_> = mail.attachments.iter().map(|part| part.section.as_deref()).collect();
    assert_eq!(sections, [Some("TEXT")], "the reader names the part left out");
    assert!(mail.attachments[0].bytes.is_empty());
}

#[test]
fn a_related_start_may_name_a_multipart() {
    let structure = format!(
        "((\"IMAGE\" \"PNG\" NIL \"<logo>\" NIL \"BASE64\" 12 NIL NIL NIL NIL)\
         ({TEXT}{HTML} \"ALTERNATIVE\" (\"BOUNDARY\" \"b2\") NIL NIL NIL) \
         \"RELATED\" (\"BOUNDARY\" \"r1\" \"START\" \"<root>\") NIL NIL NIL)"
    );
    let node = node(&structure);
    assert!(node.complete());
    let mail = rebuilt(
        &node,
        &[
            (
                "HEADER",
                "MIME-Version: 1.0\nContent-Type: multipart/related; boundary=\"r1\"; \
                 start=\"<root>\"\n\n",
            ),
            (
                "1.MIME",
                "Content-Type: image/png\nContent-ID: <logo>\n\
                 Content-Transfer-Encoding: base64\n\n",
            ),
            ("1", "iVBORw0KGgo=\n"),
            (
                "2.MIME",
                "Content-Type: multipart/alternative; boundary=\"b2\"\nContent-ID: <root>\n\n",
            ),
            ("2.1.MIME", "Content-Type: text/plain\n\n"),
            ("2.1", "hello\n"),
            ("2.2.MIME", "Content-Type: text/html\n\n"),
            ("2.2", "<p>hello</p>\n"),
        ],
    );
    assert_eq!(mail.text.as_deref().map(str::trim_end), Some("hello"));
    assert!(mail.html.is_some());
}
