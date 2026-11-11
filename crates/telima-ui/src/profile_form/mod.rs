//! Profile create/edit form. Placeholder: a later node fills this folder.
use gpui_kit::*;

#[derive(Default)]
pub struct ProfileForm;

impl Render for ProfileForm {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

#[cfg(test)]
mod tests;
