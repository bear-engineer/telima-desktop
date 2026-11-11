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
        match raw.to_ascii_lowercase().as_str() {
            "created" => Self::Created,
            "running" => Self::Running,
            "paused" => Self::Paused,
            "restarting" => Self::Restarting,
            "exited" => Self::Exited,
            "dead" => Self::Dead,
            _ => Self::Other,
        }
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
        self.tags.iter().all(|t| t == "<none>:<none>")
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
        self.ref_count == Some(0)
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
    fn only_stopped_containers_can_start() {
        use ContainerState::*;
        for s in [Created, Exited] {
            assert!(s.can_start(), "{s:?}");
        }
        for s in [Running, Paused, Restarting, Dead, Other] {
            assert!(!s.can_start(), "{s:?}");
        }
    }

    #[test]
    fn only_live_containers_can_stop() {
        use ContainerState::*;
        for s in [Running, Paused, Restarting] {
            assert!(s.can_stop(), "{s:?}");
        }
        for s in [Created, Exited, Dead, Other] {
            assert!(!s.can_stop(), "{s:?}");
        }
    }

    #[test]
    fn port_is_published_only_with_a_host_port() {
        let port = |host_port| PortBinding {
            host_ip: None,
            host_port,
            container_port: 80,
            protocol: PortProtocol::Tcp,
        };
        assert!(port(Some(8080)).is_published());
        assert!(!port(None).is_published());
    }

    #[test]
    fn parses_port_protocols() {
        assert_eq!(PortProtocol::parse("tcp"), Some(PortProtocol::Tcp));
        assert_eq!(PortProtocol::parse("UDP"), Some(PortProtocol::Udp));
        assert_eq!(PortProtocol::parse("sctp"), Some(PortProtocol::Sctp));
        assert_eq!(PortProtocol::parse("icmp"), None);
    }

    #[test]
    fn container_carries_ports_and_times_and_logs_carry_a_timestamp() {
        let c = Container {
            id: "abc".into(),
            name: "web".into(),
            image: "nginx".into(),
            state: ContainerState::Running,
            status_text: "Up 2 hours".into(),
            ports: vec![PortBinding {
                host_ip: Some("0.0.0.0".into()),
                host_port: Some(8080),
                container_port: 80,
                protocol: PortProtocol::Tcp,
            }],
            created_at: Some(UnixMillis::from_secs(10)),
            started_at: None,
        };
        assert_eq!(c.ports[0].container_port, 80);
        assert!(c.created_at > c.started_at);
        let line = LogLine {
            stream: LogStream::Stdout,
            text: "hello".into(),
            timestamp: Some(UnixMillis(1_500)),
        };
        assert_eq!(line.timestamp.map(UnixMillis::as_secs), Some(1));
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
