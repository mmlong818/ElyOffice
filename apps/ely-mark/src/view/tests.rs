use std::{cell::RefCell, rc::Rc};

use serde_json::Value;

#[gpui::test]
fn each_owed_render_ends_once_with_the_reason(cx: &mut gpui::TestAppContext) {
    let got = Rc::new(RefCell::new(Vec::<(u8, Result<Value, String>)>::new()));
    let pending = (0..2)
        .map(|n| {
            let got = got.clone();
            Box::new(move |rendered, _: &mut gpui::App| got.borrow_mut().push((n, rendered))) as super::Rendered
        })
        .collect();
    cx.update(|cx| super::end_renders(pending, "denied", cx));
    cx.run_until_parked();
    let denied = || Err("denied".to_string());
    assert_eq!(*got.borrow(), [(0, denied()), (1, denied())]);
}
