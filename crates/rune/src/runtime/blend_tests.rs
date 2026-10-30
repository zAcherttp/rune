use crate::{Cx, El, Env, Runtime, col};

const PAGE: &str = "#336699";

fn clear(view: fn(&(), &mut Cx<()>) -> El) -> [f32; 4] {
    let mut rt = Runtime::new((), view, (200.0, 100.0), Env { system_dark: true, scale: 1.0 });
    rt.set_srgb_blend(false);
    rt.frame(0.0);
    rt.clear_color()
}

fn page() -> [f32; 4] {
    let c = crate::color::hex(PAGE).unwrap();
    [c.r, c.g, c.b, 1.0]
}

fn near(a: [f32; 4], b: [f32; 4]) -> bool { a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-4) }

#[test]
fn a_root_without_fill_clears_to_the_page_that_covers_the_window() {
    let c = clear(|_, _| col().w_full().h_full().child(col().key("page").w_full().h_full().fill(crate::color::hex(PAGE).unwrap())));
    assert!(near(c, page()), "{c:?}");
}

#[test]
fn the_root_fill_still_wins() {
    let c = clear(|_, _| col().w_full().h_full().fill(crate::color::hex(PAGE).unwrap())
        .child(col().w_full().h_full().fill(crate::color::hex("#ff0000").unwrap())));
    assert!(near(c, page()), "{c:?}");
}

#[test]
fn a_fill_that_leaves_part_of_the_window_bare_is_not_the_background() {
    let c = clear(|_, _| col().w_full().h_full().child(col().w(50.0).h_full().fill(crate::color::hex(PAGE).unwrap())));
    assert_eq!(c, [0.0, 0.0, 0.0, 1.0]);
}
