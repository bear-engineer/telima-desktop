//! Contract test against a real Docker engine. Run with:
//! `TELIMA_DOCKER_SOCKET=<socket> cargo test -p telima-docker --test contract -- --ignored`
//! Uses only uniquely named throwaway resources and always cleans up.

use bollard::Docker;
use bollard::models::{ContainerCreateBody, HostConfig, VolumeCreateRequest};
use bollard::query_parameters::{
    CreateContainerOptionsBuilder, CreateImageOptionsBuilder, RemoveContainerOptionsBuilder,
    RemoveVolumeOptionsBuilder, StartContainerOptions, WaitContainerOptions,
};
use futures_util::TryStreamExt;
use telima_core::container::LogStream;
use telima_core::ports::{ContainerEngine, PortError};
use telima_docker::DockerEngine;
use telima_docker::map::supports_volume_sizes;

const IMAGE: &str = "alpine:3";

fn socket() -> String {
    std::env::var("TELIMA_DOCKER_SOCKET")
        .expect("TELIMA_DOCKER_SOCKET must point at a docker socket")
}

fn unique(prefix: &str) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{prefix}-{}-{nanos}", std::process::id())
}

#[tokio::test]
#[ignore = "needs a running docker engine (TELIMA_DOCKER_SOCKET)"]
async fn docker_engine_contract() {
    let socket = socket();
    let engine = DockerEngine::connect(&socket).expect("connect");
    let raw = Docker::connect_with_unix(&socket, 120, bollard::API_DEFAULT_VERSION).expect("raw");
    let name = unique("telima-contract");
    let server = raw.version().await.expect("version");
    println!(
        "docker server version {:?}, API {:?}",
        server.version, server.api_version
    );

    raw.create_image(
        Some(
            CreateImageOptionsBuilder::default()
                .from_image(IMAGE)
                .build(),
        ),
        None,
        None,
    )
    .try_collect::<Vec<_>>()
    .await
    .expect("pull alpine");

    let result = async {
        raw.create_container(
            Some(CreateContainerOptionsBuilder::default().name(&name).build()),
            ContainerCreateBody {
                image: Some(IMAGE.into()),
                cmd: Some(vec![
                    "sh".into(),
                    "-c".into(),
                    "echo hello-out; echo hello-err 1>&2".into(),
                ]),
                ..Default::default()
            },
        )
        .await
        .expect("create container");
        raw.start_container(&name, None::<StartContainerOptions>)
            .await
            .expect("start");
        raw.wait_container(&name, None::<WaitContainerOptions>)
            .try_collect::<Vec<_>>()
            .await
            .expect("wait");

        let listed = engine.containers().await.expect("containers");
        assert!(listed.iter().any(|c| c.name == name), "container listed");

        let mut lines = Vec::new();
        engine
            .logs(&name, true, &mut |l| lines.push(l))
            .await
            .expect("logs");
        assert!(
            lines
                .iter()
                .any(|l| l.stream == LogStream::Stdout && l.text.contains("hello-out"))
        );
        assert!(
            lines
                .iter()
                .any(|l| l.stream == LogStream::Stderr && l.text.contains("hello-err"))
        );

        engine.restart_container(&name).await.expect("restart");
        assert!(!engine.images().await.expect("images").is_empty());
        engine.disk_usage().await.expect("disk usage");

        engine.remove_container(&name, true).await.expect("remove");
        assert!(matches!(
            engine.remove_container(&name, true).await,
            Err(PortError::NotFound(_))
        ));
    }
    .await;

    let _ = raw
        .remove_container(
            &name,
            Some(RemoveContainerOptionsBuilder::default().force(true).build()),
        )
        .await;
    result
}

#[tokio::test]
#[ignore = "needs a running docker engine (TELIMA_DOCKER_SOCKET)"]
async fn docker_volume_contract() {
    let socket = socket();
    let engine = DockerEngine::connect(&socket).expect("connect");
    let raw = Docker::connect_with_unix(&socket, 120, bollard::API_DEFAULT_VERSION).expect("raw");
    let vol = unique("telima-contract-vol");

    raw.create_volume(VolumeCreateRequest {
        name: Some(vol.clone()),
        ..Default::default()
    })
    .await
    .expect("create volume");

    let result = async {
        let volumes = engine.volumes().await.expect("volumes");
        assert!(volumes.iter().any(|v| v.name == vol));
        let report = engine.prune_volumes().await.expect("prune volumes");
        assert!(report.removed.iter().any(|n| n == &vol));
        assert!(matches!(
            engine.remove_volume(&vol).await,
            Err(PortError::NotFound(_))
        ));
    }
    .await;

    let _ = raw
        .remove_volume(
            &vol,
            Some(RemoveVolumeOptionsBuilder::default().force(true).build()),
        )
        .await;
    result
}

#[tokio::test]
#[ignore = "needs a running docker engine (TELIMA_DOCKER_SOCKET)"]
async fn volumes_report_size_and_ref_count() {
    let socket = socket();
    let engine = DockerEngine::connect(&socket).expect("connect");
    let raw = Docker::connect_with_unix(&socket, 120, bollard::API_DEFAULT_VERSION).expect("raw");
    let vol = unique("telima-contract-usage");
    let name = unique("telima-contract-writer");

    raw.create_image(
        Some(
            CreateImageOptionsBuilder::default()
                .from_image(IMAGE)
                .build(),
        ),
        None,
        None,
    )
    .try_collect::<Vec<_>>()
    .await
    .expect("pull alpine");
    raw.create_volume(VolumeCreateRequest {
        name: Some(vol.clone()),
        ..Default::default()
    })
    .await
    .expect("create volume");

    let result = async {
        raw.create_container(
            Some(CreateContainerOptionsBuilder::default().name(&name).build()),
            ContainerCreateBody {
                image: Some(IMAGE.into()),
                cmd: Some(vec![
                    "sh".into(),
                    "-c".into(),
                    "dd if=/dev/zero of=/data/blob bs=1024 count=256".into(),
                ]),
                host_config: Some(HostConfig {
                    binds: Some(vec![format!("{vol}:/data")]),
                    ..Default::default()
                }),
                ..Default::default()
            },
        )
        .await
        .expect("create container");
        raw.start_container(&name, None::<StartContainerOptions>)
            .await
            .expect("start");
        raw.wait_container(&name, None::<WaitContainerOptions>)
            .try_collect::<Vec<_>>()
            .await
            .expect("wait");

        let volumes = engine.volumes().await.expect("volumes");
        let found = volumes.iter().find(|v| v.name == vol).expect("listed");
        // Reference counts come from container mounts on every engine.
        assert_eq!(found.ref_count, Some(1), "ref count: {found:?}");
        // Sizes need `df` in the API >= 1.52 shape (see `VOLUME_SIZE_MIN_API`).
        let server_api = raw
            .version()
            .await
            .expect("version")
            .api_version
            .unwrap_or_default();
        println!("docker server API {server_api}");
        if supports_volume_sizes(&server_api) {
            assert!(
                found.size_bytes.is_some_and(|s| s > 0),
                "size known on API {server_api}: {found:?}"
            );
        } else {
            assert_eq!(
                found.size_bytes, None,
                "no sizes before API 1.52: {found:?}"
            );
        }
    }
    .await;

    let _ = raw
        .remove_container(
            &name,
            Some(RemoveContainerOptionsBuilder::default().force(true).build()),
        )
        .await;
    let _ = raw
        .remove_volume(
            &vol,
            Some(RemoveVolumeOptionsBuilder::default().force(true).build()),
        )
        .await;
    result
}

#[tokio::test]
#[ignore = "needs a running docker engine (TELIMA_DOCKER_SOCKET)"]
async fn missing_socket_is_unavailable() {
    let err = DockerEngine::connect("/nonexistent/telima.sock").err();
    assert!(matches!(err, Some(PortError::Unavailable(_))));
}

#[tokio::test]
#[ignore = "needs a running docker engine (TELIMA_DOCKER_SOCKET)"]
async fn start_stop_ports_and_log_timestamps() {
    use bollard::models::PortBinding as BollardPort;
    use telima_core::container::{ContainerState, PortProtocol};

    let socket = socket();
    let engine = DockerEngine::connect(&socket).expect("connect");
    let raw = Docker::connect_with_unix(&socket, 120, bollard::API_DEFAULT_VERSION).expect("raw");
    let name = unique("telima-contract-life");

    raw.create_image(
        Some(
            CreateImageOptionsBuilder::default()
                .from_image(IMAGE)
                .build(),
        ),
        None,
        None,
    )
    .try_collect::<Vec<_>>()
    .await
    .expect("pull alpine");

    let result = async {
        raw.create_container(
            Some(CreateContainerOptionsBuilder::default().name(&name).build()),
            ContainerCreateBody {
                image: Some(IMAGE.into()),
                cmd: Some(vec![
                    "sh".into(),
                    "-c".into(),
                    "echo stamped-line; sleep 300".into(),
                ]),
                exposed_ports: Some(vec!["8080/tcp".into()]),
                host_config: Some(HostConfig {
                    port_bindings: Some(
                        [(
                            "8080/tcp".to_owned(),
                            Some(vec![BollardPort {
                                host_ip: Some("127.0.0.1".into()),
                                host_port: Some("0".into()),
                            }]),
                        )]
                        .into(),
                    ),
                    ..Default::default()
                }),
                ..Default::default()
            },
        )
        .await
        .expect("create container");

        let find = |list: Vec<telima_core::container::Container>| {
            list.into_iter().find(|c| c.name == name).expect("listed")
        };
        let created = find(engine.containers().await.expect("containers"));
        assert_eq!(created.state, ContainerState::Created);
        assert!(created.created_at.is_some(), "created_at: {created:?}");
        assert_eq!(created.started_at, None);

        engine.start_container(&name).await.expect("start");
        let running = find(engine.containers().await.expect("containers"));
        assert_eq!(running.state, ContainerState::Running);
        assert!(running.started_at.is_some(), "started_at: {running:?}");
        let published = running
            .ports
            .iter()
            .find(|p| p.container_port == 8080)
            .expect("port 8080 listed");
        assert_eq!(published.protocol, PortProtocol::Tcp);
        assert!(published.host_port.is_some_and(|p| p > 0), "{published:?}");

        let mut lines = Vec::new();
        engine
            .logs(&name, false, &mut |l| lines.push(l))
            .await
            .expect("logs");
        let line = lines
            .iter()
            .find(|l| l.text == "stamped-line")
            .unwrap_or_else(|| panic!("log line without prefix leftovers: {lines:?}"));
        assert!(line.timestamp.is_some(), "timestamp: {line:?}");

        engine.stop_container(&name).await.expect("stop");
        // Older engines (Docker 28 on CI) answer the stop call before the
        // container list reflects it, so poll the list briefly.
        let mut stopped = find(engine.containers().await.expect("containers"));
        for _ in 0..20 {
            if stopped.state == ContainerState::Exited {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            stopped = find(engine.containers().await.expect("containers"));
        }
        assert_eq!(stopped.state, ContainerState::Exited);
        engine.remove_container(&name, true).await.expect("remove");
    }
    .await;

    let _ = raw
        .remove_container(
            &name,
            Some(RemoveContainerOptionsBuilder::default().force(true).build()),
        )
        .await;
    result
}
