//! Contract test against a real Docker engine. Run with:
//! `TELIMA_DOCKER_SOCKET=<socket> cargo test -p telima-docker --test contract -- --ignored`
//! Uses only uniquely named throwaway resources and always cleans up.

use bollard::Docker;
use bollard::models::{ContainerCreateBody, VolumeCreateRequest};
use bollard::query_parameters::{
    CreateContainerOptionsBuilder, CreateImageOptionsBuilder, RemoveContainerOptionsBuilder,
    RemoveVolumeOptionsBuilder, StartContainerOptions, WaitContainerOptions,
};
use futures_util::TryStreamExt;
use telima_core::container::LogStream;
use telima_core::ports::{ContainerEngine, PortError};
use telima_docker::DockerEngine;

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
async fn missing_socket_is_unavailable() {
    let err = DockerEngine::connect("/nonexistent/telima.sock").err();
    assert!(matches!(err, Some(PortError::Unavailable(_))));
}
