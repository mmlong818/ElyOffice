use std::{
    io::{BufRead, BufReader, Write},
    net::TcpListener,
    thread,
};

use ely_mail_engine::{Error, fetch};

/// Answers one request with `kind` and `body`.
fn serve(kind: &'static str, body: Vec<u8>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut line = String::new();
        while reader.read_line(&mut line).unwrap() > 2 {
            line.clear();
        }
        let mut stream = stream;
        let head = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: {kind}\r\nContent-Length: {}\r\n\r\n",
            body.len()
        );
        stream.write_all(head.as_bytes()).unwrap();
        // The client may stop reading a body it refuses.
        stream.write_all(&body).ok();
    });
    format!("http://127.0.0.1:{port}/p.png")
}

#[test]
fn a_web_picture_comes_with_its_type() {
    let url = serve("image/PNG; charset=binary", b"\x89PNG".to_vec());
    let (kind, bytes) = fetch::picture(&url).unwrap();
    assert_eq!(kind, "image/png");
    assert_eq!(bytes, b"\x89PNG");
}

#[test]
fn only_web_pictures_under_the_limit_are_taken() {
    let page = serve("text/html", b"<p>no</p>".to_vec());
    let refused = |result: ely_mail_engine::Result<_>| matches!(result, Err(Error::Fetch(_)));
    assert!(refused(fetch::picture(&page)));
    assert!(refused(fetch::picture("file:///etc/passwd")));
    let huge = serve("image/png", vec![0; fetch::LARGEST as usize + 1]);
    assert!(refused(fetch::picture(&huge)));
}
