//! Containers / images / volumes views of one profile. Placeholder: a later node fills this folder.
use gpui_kit::*;

#[derive(Default)]
pub struct EngineViews;

impl Render for EngineViews {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}
