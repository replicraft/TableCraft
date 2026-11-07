//! Unoccupied datasheet space is ruled decoration, never additional records.
use egui::{Color32, Rect, Shape, Stroke, pos2};

const EMPTY_RULE_OPACITY: f32 = 0.4;

pub(crate) fn shape(
    columns: &[Rect],
    viewport: Rect,
    content_bottom: f32,
    row_height: f32,
    color: Color32,
) -> Shape {
    let (Some(first), Some(last)) = (columns.first(), columns.last()) else {
        return Shape::Noop;
    };
    let bounds = Rect::from_min_max(
        pos2(first.left(), content_bottom.max(viewport.top())),
        pos2(last.right(), viewport.bottom()),
    )
    .intersect(viewport);
    if !bounds.is_positive() || !row_height.is_finite() || row_height <= 0.0 {
        return Shape::Noop;
    }

    let stroke = Stroke::new(0.5, color.gamma_multiply(EMPTY_RULE_OPACITY));
    let mut lines = Vec::new();
    for column in columns {
        let x = column.right();
        if (bounds.left()..=bounds.right()).contains(&x) {
            lines.push(Shape::line_segment(
                [pos2(x, bounds.top()), pos2(x, bounds.bottom())],
                stroke,
            ));
        }
    }

    // Keep row pitch tied to the real rows even if the viewport starts midway
    // through a page. No table allocation or hit target accompanies these lines.
    let skipped = ((bounds.top() - content_bottom) / row_height).ceil();
    let mut y = content_bottom + skipped * row_height;
    while y <= bounds.bottom() {
        lines.push(Shape::line_segment(
            [pos2(bounds.left(), y), pos2(bounds.right(), y)],
            stroke,
        ));
        y += row_height;
    }
    Shape::Vec(lines)
}

#[cfg(test)]
mod tests;
