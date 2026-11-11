//! Bundled IBM Plex fonts (SIL OFL 1.1, see assets/fonts/OFL.txt), so `theme::font::SANS` and
//! `theme::font::MONO` render the same on every machine.

/// (file name, TrueType bytes) for every bundled face.
pub const FONTS: [(&str, &[u8]); 5] = [
    (
        "IBMPlexSansKR-Regular.ttf",
        include_bytes!("../assets/fonts/IBMPlexSansKR-Regular.ttf"),
    ),
    (
        "IBMPlexSansKR-Medium.ttf",
        include_bytes!("../assets/fonts/IBMPlexSansKR-Medium.ttf"),
    ),
    (
        "IBMPlexSansKR-SemiBold.ttf",
        include_bytes!("../assets/fonts/IBMPlexSansKR-SemiBold.ttf"),
    ),
    (
        "IBMPlexMono-Regular.ttf",
        include_bytes!("../assets/fonts/IBMPlexMono-Regular.ttf"),
    ),
    (
        "IBMPlexMono-Medium.ttf",
        include_bytes!("../assets/fonts/IBMPlexMono-Medium.ttf"),
    ),
];
