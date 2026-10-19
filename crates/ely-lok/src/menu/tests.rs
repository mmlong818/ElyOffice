use super::*;

fn item(command: &str, enabled: bool) -> String {
    format!(r#"{{"text":"x","type":"command","command":"{command}","enabled":"{enabled}"}}"#)
}

#[test]
fn spelling_rows_come_first_class() {
    let menu = r#"{"menu":[{"text":"The","type":"command","command":".uno:SpellCheckApplySuggestion?ApplyRule:string=Spelling_The","enabled":"true"},{"type":"separator"},{"text":"Ignore All","type":"command","command":".uno:SpellCheckIgnoreAll?Type:string=Spelling","enabled":"true"}]}"#;
    let offered = read(menu).expect("the menu parses");
    let spelling = offered.spelling.expect("a spelling menu");
    assert_eq!(spelling.suggestions, [("The".into(), "Spelling_The".into())]);
    assert_eq!(offered.target, Target::Text);
    let other = format!(r#"{{"menu":[{}]}}"#, item(".uno:Cut", true));
    assert_eq!(read(&other).expect("the menu parses").spelling, None);
}

#[test]
fn the_menu_names_its_target_and_flags() {
    let table = format!(
        r#"{{"menu":[{},{{"text":"Insert","type":"menu","command":".uno:TableInsertMenu","menu":[{}],"enabled":"true"}}]}}"#,
        item(".uno:MergeCells", false),
        item(".uno:InsertRowsBefore", true),
    );
    let table = read(&table).expect("the table menu parses");
    assert_eq!(table.target, Target::Table);
    assert!(table.allows(".uno:InsertRowsBefore"), "submenu rows count");
    assert!(!table.allows(".uno:MergeCells"), "a disabled row");
    assert!(!table.allows(".uno:DeleteTable"), "an absent row");
    let frame = format!(
        r#"{{"menu":[{{"type":"menu","menu":[{}],"enabled":"true"}}]}}"#,
        item(".uno:WrapMenu", true)
    );
    assert_eq!(read(&frame).expect("a commandless submenu").target, Target::Frame);
    let cell = format!(r#"{{"menu":[{}]}}"#, item(".uno:InsertCell", false));
    assert_eq!(read(&cell).expect("parses").target, Target::Cell);
    let object = format!(r#"{{"menu":[{}]}}"#, item(".uno:ArrangeMenu", true));
    assert_eq!(read(&object).expect("parses").target, Target::Object);
    let text = format!(r#"{{"menu":[{}]}}"#, item(".uno:Paste", true));
    assert_eq!(read(&text).expect("parses").target, Target::Text);
    // Impress: a picture also arranges; a bare slide has layouts.
    let picture = format!(r#"{{"menu":[{},{}]}}"#, item(".uno:ChangePicture", true), item(".uno:ArrangeMenu", true));
    assert_eq!(read(&picture).expect("parses").target, Target::Picture);
    let slide = format!(r#"{{"menu":[{},{}]}}"#, item(".uno:Paste", true), item(".uno:SlideLayoutMenu", true));
    assert_eq!(read(&slide).expect("parses").target, Target::Slide);
}

#[test]
fn a_malformed_menu_fails() {
    assert!(read(r#"{"menu":[{"type":"command","command":".uno:Cut"}]}"#).is_err());
    assert!(read(r#"{"menu":[{"type":"command","enabled":"true"}]}"#).is_err());
    assert!(read(r#"{"items":[]}"#).is_err());
}

#[test]
fn command_states_answer_each_asked_command() {
    let json = r#"{"commandStates":{".uno:DeleteRows":false,".uno:InsertRowsBefore":true}}"#;
    let runs = read_runs(json, &[".uno:InsertRowsBefore", ".uno:DeleteRows"]).expect("parses");
    assert!(runs.allows(".uno:InsertRowsBefore"));
    assert!(!runs.allows(".uno:DeleteRows"));
    assert!(read_runs(json, &[".uno:HideRow"]).is_err(), "an unanswered command");
    let text = r#"{"commandStates":{".uno:Cut":"true"}}"#;
    assert!(read_runs(text, &[".uno:Cut"]).is_err(), "a state is a boolean");
}
