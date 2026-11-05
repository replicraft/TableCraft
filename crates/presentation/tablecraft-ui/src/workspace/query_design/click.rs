//! Pair pointer clicks on the same live designer control, independent of other widgets.
use egui::{Id, Pos2, Response, Ui};

pub(super) struct Pair {
    id: Id,
    time: f64,
    position: Pos2,
}

/// The stored ID is comparison-only. The caller acts on its current response/data.
pub(super) fn activate(ui: &Ui, response: &Response, previous: &mut Option<Pair>) -> bool {
    let Some(position) = response.interact_pointer_pos() else {
        *previous = None;
        return false;
    };
    let time = ui.input(|input| input.time);
    let (delay, distance) = ui.ctx().options(|options| {
        (
            options.input_options.max_double_click_delay,
            options.input_options.max_click_dist,
        )
    });
    let paired = previous.take().is_some_and(|click| {
        click.id == response.id
            && (0.0..delay).contains(&(time - click.time))
            && click.position.distance_sq(position) < distance * distance
    });
    if !paired {
        *previous = Some(Pair {
            id: response.id,
            time,
            position,
        });
    }
    paired
}
