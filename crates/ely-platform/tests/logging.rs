use ely_platform::{AppId, logging};

#[test]
fn log_lines_reach_the_app_file_and_a_second_init_fails() {
    let dir = tempfile::tempdir().unwrap();

    logging::init_in(AppId::Doc, dir.path()).unwrap();
    log::info!("hello from the test");
    log::logger().flush();

    let text = std::fs::read_to_string(dir.path().join("ely.log")).unwrap();
    assert!(text.contains(&format!("ElyDoc {} started", env!("CARGO_PKG_VERSION"))), "{text}");
    assert!(text.contains("hello from the test"), "{text}");

    let error = logging::init_in(AppId::Mail, dir.path()).unwrap_err();
    assert!(
        format!("{error:#}").contains("a logger is already set"),
        "{error:#}"
    );
}
