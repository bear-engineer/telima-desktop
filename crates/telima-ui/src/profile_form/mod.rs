//! Profile create/edit form, delete confirmation, and the start progress log.
//!
//! Each view owns an inbox like `shell::MainView`: use-case callbacks run on the spawner's
//! thread and only push a message; `pump` applies them on the GPUI thread (a poll task
//! calls it every 50 ms; tests call it directly).
use std::time::Duration;

use gpui_kit::*;

mod delete;
mod editor;
mod progress;
mod widgets;

pub use delete::{DeleteConfirm, DeleteEvent};
pub use editor::{Draft, Field, FormEvent, Mode, ProfileEditor, violation_text};
pub use progress::{ProgressEnd, StartProgress};

/// How often the views drain finished use-case results.
const POLL: Duration = Duration::from_millis(50);

/// Calls `pump` on the view every [`POLL`] until the view is dropped.
fn poll<V: 'static>(cx: &mut Context<V>, pump: fn(&mut V, &mut Context<V>)) -> Task<()> {
    cx.spawn(async move |this, cx| {
        loop {
            cx.background_executor().timer(POLL).await;
            if this.update(cx, pump).is_err() {
                break;
            }
        }
    })
}

#[cfg(test)]
mod tests;
