//! `DockerEngine`: the container-engine port over the Docker Engine API (bollard).

use std::collections::HashMap;

use bollard::errors::Error;
use bollard::query_parameters::{
    ListContainersOptionsBuilder, ListImagesOptionsBuilder, LogsOptionsBuilder,
    PruneImagesOptionsBuilder, RemoveContainerOptionsBuilder, RestartContainerOptions,
};
use bollard::{API_DEFAULT_VERSION, Docker};
use futures_util::StreamExt;
use telima_core::container::{Container, DiskUsage, Image, LogLine, PruneReport, Volume};
use telima_core::ports::{ContainerEngine, PortError, PortResult};

use crate::map;

const TIMEOUT_SECS: u64 = 120;

/// Translate a bollard error into the port's error vocabulary.
fn map_error(err: Error) -> PortError {
    match err {
        Error::SocketNotFoundError(msg) => PortError::Unavailable(msg),
        Error::DockerResponseServerError {
            status_code: 404,
            message,
        } => PortError::NotFound(message),
        Error::IOError { err }
            if matches!(
                err.kind(),
                std::io::ErrorKind::ConnectionRefused
                    | std::io::ErrorKind::NotFound
                    | std::io::ErrorKind::ConnectionReset
                    | std::io::ErrorKind::PermissionDenied
            ) =>
        {
            PortError::Unavailable(err.to_string())
        }
        Error::HyperLegacyError { err } if err.is_connect() => {
            PortError::Unavailable(err.to_string())
        }
        other => PortError::Failed(other.to_string()),
    }
}

/// Docker Engine API client bound to one engine socket (one Colima profile).
#[derive(Debug, Clone)]
pub struct DockerEngine {
    docker: Docker,
}

impl DockerEngine {
    /// Connect to the engine's unix socket. The socket must exist; the engine itself is
    /// only contacted on the first call.
    pub fn connect(socket: &str) -> PortResult<Self> {
        Docker::connect_with_unix(socket, TIMEOUT_SECS, API_DEFAULT_VERSION)
            .map(|docker| Self { docker })
            .map_err(map_error)
    }
}

impl ContainerEngine for DockerEngine {
    async fn containers(&self) -> PortResult<Vec<Container>> {
        let options = ListContainersOptionsBuilder::default().all(true).build();
        let list = self
            .docker
            .list_containers(Some(options))
            .await
            .map_err(map_error)?;
        Ok(list.iter().map(map::map_container).collect())
    }

    async fn restart_container(&self, id: &str) -> PortResult<()> {
        self.docker
            .restart_container(id, None::<RestartContainerOptions>)
            .await
            .map_err(map_error)
    }

    async fn remove_container(&self, id: &str, force: bool) -> PortResult<()> {
        let options = RemoveContainerOptionsBuilder::default()
            .force(force)
            .build();
        self.docker
            .remove_container(id, Some(options))
            .await
            .map_err(map_error)
    }

    async fn logs(
        &self,
        id: &str,
        follow: bool,
        sink: &mut (dyn FnMut(LogLine) + Send),
    ) -> PortResult<()> {
        let options = LogsOptionsBuilder::default()
            .follow(follow)
            .stdout(true)
            .stderr(true)
            .build();
        let mut stream = self.docker.logs(id, Some(options));
        while let Some(chunk) = stream.next().await {
            let Some(line) = map::map_log_output(&chunk.map_err(map_error)?) else {
                continue;
            };
            // A chunk may hold several lines; the sink gets one line each.
            for text in line.text.split('\n') {
                sink(LogLine {
                    stream: line.stream,
                    text: text.to_owned(),
                });
            }
        }
        Ok(())
    }

    async fn images(&self) -> PortResult<Vec<Image>> {
        let options = ListImagesOptionsBuilder::default().build();
        let list = self
            .docker
            .list_images(Some(options))
            .await
            .map_err(map_error)?;
        Ok(list.iter().map(map::map_image).collect())
    }

    async fn remove_image(&self, id: &str) -> PortResult<()> {
        self.docker
            .remove_image(
                id,
                None::<bollard::query_parameters::RemoveImageOptions>,
                None,
            )
            .await
            .map(|_| ())
            .map_err(map_error)
    }

    async fn prune_images(&self) -> PortResult<PruneReport> {
        let filters = HashMap::from([("dangling".to_owned(), vec!["true".to_owned()])]);
        let options = PruneImagesOptionsBuilder::default()
            .filters(&filters)
            .build();
        let response = self
            .docker
            .prune_images(Some(options))
            .await
            .map_err(map_error)?;
        Ok(map::map_image_prune(&response))
    }

    async fn volumes(&self) -> PortResult<Vec<Volume>> {
        let response = self
            .docker
            .list_volumes(None::<bollard::query_parameters::ListVolumesOptions>)
            .await
            .map_err(map_error)?;
        Ok(response
            .volumes
            .unwrap_or_default()
            .iter()
            .map(map::map_volume)
            .collect())
    }

    async fn remove_volume(&self, name: &str) -> PortResult<()> {
        self.docker
            .remove_volume(name, None::<bollard::query_parameters::RemoveVolumeOptions>)
            .await
            .map_err(map_error)
    }

    async fn prune_volumes(&self) -> PortResult<PruneReport> {
        let response = self
            .docker
            .prune_volumes(None::<bollard::query_parameters::PruneVolumesOptions>)
            .await
            .map_err(map_error)?;
        Ok(map::map_volume_prune(&response))
    }

    async fn disk_usage(&self) -> PortResult<DiskUsage> {
        let response = self
            .docker
            .df(None::<bollard::query_parameters::DataUsageOptions>)
            .await
            .map_err(map_error)?;
        Ok(map::map_disk_usage(&response))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bollard::errors::Error;

    fn server_error(status_code: u16) -> Error {
        Error::DockerResponseServerError {
            status_code,
            message: "boom".into(),
        }
    }

    #[test]
    fn status_404_is_not_found() {
        assert!(matches!(
            map_error(server_error(404)),
            PortError::NotFound(_)
        ));
    }

    #[test]
    fn other_status_is_failed() {
        assert!(matches!(map_error(server_error(500)), PortError::Failed(_)));
    }

    #[test]
    fn missing_socket_is_unavailable() {
        let err = Error::SocketNotFoundError("/x".into());
        assert!(matches!(map_error(err), PortError::Unavailable(_)));
    }

    #[test]
    fn refused_connection_is_unavailable() {
        let io = std::io::Error::from(std::io::ErrorKind::ConnectionRefused);
        assert!(matches!(
            map_error(Error::IOError { err: io }),
            PortError::Unavailable(_)
        ));
    }

    #[test]
    fn other_io_error_is_failed() {
        let io = std::io::Error::from(std::io::ErrorKind::InvalidData);
        assert!(matches!(
            map_error(Error::IOError { err: io }),
            PortError::Failed(_)
        ));
    }

    #[test]
    fn connect_to_missing_socket_is_unavailable() {
        let err = DockerEngine::connect("/nonexistent/telima-test.sock").unwrap_err();
        assert!(matches!(err, PortError::Unavailable(_)));
    }
}
