//! Container-engine entities.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerState {
    Created,
    Running,
    Paused,
    Restarting,
    Exited,
    Dead,
    Other,
}

impl ContainerState {
    /// Map an engine state word (`running`, `exited`, ...) to a state.
    pub fn parse(raw: &str) -> Self {
        let _ = raw;
        todo!()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Container {
    pub id: String,
    pub name: String,
    pub image: String,
    pub state: ContainerState,
    pub status_text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Image {
    pub id: String,
    pub tags: Vec<String>,
    pub size_bytes: u64,
    /// Number of containers (running or not) using the image, when known.
    pub containers: Option<u64>,
}

impl Image {
    /// An image with no tag (shown as `<none>`).
    pub fn is_dangling(&self) -> bool {
        todo!()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Volume {
    pub name: String,
    pub size_bytes: Option<u64>,
    /// Number of containers referencing the volume, when known.
    pub ref_count: Option<u64>,
}

impl Volume {
    /// True only when the engine reports zero references; unknown counts are treated as in use.
    pub fn is_unused(&self) -> bool {
        todo!()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DiskUsage {
    pub images_bytes: u64,
    pub containers_bytes: u64,
    pub volumes_bytes: u64,
    pub build_cache_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PruneReport {
    pub removed: Vec<String>,
    pub reclaimed_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogLine {
    pub stream: LogStream,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogStream {
    Stdout,
    Stderr,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_engine_states() {
        assert_eq!(ContainerState::parse("running"), ContainerState::Running);
        assert_eq!(ContainerState::parse("exited"), ContainerState::Exited);
        assert_eq!(ContainerState::parse("Paused"), ContainerState::Paused);
        assert_eq!(ContainerState::parse("weird"), ContainerState::Other);
    }

    #[test]
    fn dangling_means_no_tags() {
        let img = |tags: Vec<&str>| Image {
            id: "sha".into(),
            tags: tags.into_iter().map(String::from).collect(),
            size_bytes: 1,
            containers: None,
        };
        assert!(img(vec![]).is_dangling());
        assert!(img(vec!["<none>:<none>"]).is_dangling());
        assert!(!img(vec!["alpine:3"]).is_dangling());
    }

    #[test]
    fn unknown_ref_count_is_not_unused() {
        let vol = |r| Volume {
            name: "v".into(),
            size_bytes: None,
            ref_count: r,
        };
        assert!(vol(Some(0)).is_unused());
        assert!(!vol(Some(1)).is_unused());
        assert!(!vol(None).is_unused());
    }
}
