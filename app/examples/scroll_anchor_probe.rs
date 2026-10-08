//! PUBLIC pure anchors/operation unit tests only; no App, worker or Host.
#[path = "../src/scroll_anchor.rs"]
mod scroll_anchor;
#[path = "../src/scroll_operation.rs"]
mod scroll_operation;
fn main() {
    let anchor = scroll_anchor::Anchor::capture(&[8, 2, 9], &[87.0, 145.0, 40.0], 99.0).unwrap();
    assert_eq!(
        anchor.restore(&[11, 8, 2, 9], &[160.0, 87.0, 87.0, 40.0], 50.0, 0.0),
        Some(259.0)
    );
    let _ = scroll_operation::restore::<()>("PUBLIC-absent-probe".into(), 0.0, |_, _| ());
    println!(
        "PUBLIC source-only anchor/operation probe passed; no runtime or rendering qualification"
    );
}
