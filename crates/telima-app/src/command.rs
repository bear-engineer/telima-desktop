//! Commands the tray menu and the window toolbar send to the application.

use telima_core::profile::ProfileName;

/// One user intent from the tray menu or the toolbar. The binary applies it to [`crate::runtime::App`]
/// and the window; the models in [`crate::view::tray`] attach one to every clickable item.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AppCommand {
    /// Bring the window to the front, or open it again after it was closed.
    OpenWindow,
    Start(ProfileName),
    Stop(ProfileName),
    Restart(ProfileName),
    /// Read the profile list again.
    Refresh,
    /// Open the new-profile form.
    NewProfile,
    Quit,
}
