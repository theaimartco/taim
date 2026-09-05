// The smallest possible real Taim application: a window showing one piece
// of static content. Exists to measure Pillar XIII's "trivial app under
// 1.5 MiB stripped" gate (future.md) -- see `examples/mobile_shell` for a
// realistic app and this crate's `Cargo.toml` for why the feature list
// matches it.
use taim::widget::text;
use taim::Element;

fn view(_state: &()) -> Element<'_, ()> {
    text("Hello, Taim!").size(24).into()
}

pub fn main() -> taim::Result {
    taim::application(|| (), |_state: &mut (), _message: ()| {}, view)
        .title("Taim — Trivial")
        .run()
}
