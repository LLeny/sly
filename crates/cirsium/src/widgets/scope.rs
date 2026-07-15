use gpui::{Bounds, Hsla, Pixels, Window, fill, point, px, size};

pub(crate) fn scope_baseline(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    axis: Hsla,
) -> (f32, f32, f32, f32) {
    let w = f32::from(bounds.size.width);
    let h = f32::from(bounds.size.height);
    let ox = f32::from(bounds.origin.x);
    let oy = f32::from(bounds.origin.y);
    let mid = oy + h / 2.0;
    let amp = (h / 2.0 - 1.0).max(0.0);

    window.paint_quad(fill(
        Bounds::new(point(px(ox), px(mid)), size(px(w), px(1.0))),
        axis,
    ));

    (ox, w, mid, amp)
}
