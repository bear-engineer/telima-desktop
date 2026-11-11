//! `DockerEngine`: the container-engine port over the Docker Engine API (bollard).

use std::collections::HashMap;

use bollard::errors::Error;
use bollard::query_parameters::{
    ListContainersOptionsBuilder, ListImagesOptionsBuilder, LogsOptionsBuilder,
    PruneImagesOptionsBuilder, PruneVolumesOptionsBuilder, RemoveContainerOptionsBuilder,
    RestartContainerOptions,
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

/// Volume prune filters. Docker API >= 1.42 prunes only anonymous volumes unless
/// `all=true`; the app's prune preview (`Volume::is_unused`) counts named volumes too.
fn prune_volume_filters() -> HashMap<String, Vec<String>> {
    HashMap::from([("all".to_owned(), vec!["true".to_owned()])])
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
        let filters = prune_volume_filters();
        let options = PruneVolumesOptionsBuilder::default()
            .filters(&filters)
            .build();
        let response = self
            .docker
            .prune_volumes(Some(options))
            .await
            .map_err(map_error)?;
        Ok(map::map_volume_prune(&response))
    }

    async fn disk_usage(&self) -> PortResult<DiskUsage> {
        let mut attempt = 0;
        loop {
            match self
                .docker
                .df(None::<bollard::query_parameters::DataUsageOptions>)
                .await
            {
                Ok(response) => return Ok(map::map_disk_usage(&response)),
                Err(err) => match disk_usage_retry_delay(attempt, &err) {
                    Some(delay) => {
                        attempt += 1;
                        tokio::time::sleep(delay).await;
                    }
                    None => return Err(map_error(err)),
                },
            }
        }
    }
}

/// Total tries for `df` (first call + retries).
const DISK_USAGE_MAX_ATTEMPTS: u32 = 5;

/// Delay before the next `df` try after `attempt` failures (0-based), or
/// `None` when the error is not transient or the budget is spent.
/// `df` walks every container's rw layer; the engine answers 5xx while a
/// just-restarted container's layer is not registered yet, which settles
/// within seconds.
fn disk_usage_retry_delay(
    attempt: u32,
    err: &bollard::errors::Error,
) -> Option<std::time::Duration> {
    let transient = matches!(
        err,
        bollard::errors::Error::DockerResponseServerError { status_code, .. }
            if (500..600).contains(status_code)
    );
    if !transient || attempt + 1 >= DISK_USAGE_MAX_ATTEMPTS {
        return None;
    }
    Some(std::time::Duration::from_millis(250 << attempt))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bollard::errors::Error;

    #[test]
    fn df_retries_5xx_with_growing_backoff() {
        let delays: Vec<_> = (0..4)
            .map(|a| disk_usage_retry_delay(a, &server_error(500)))
            .collect();
        let ms: Vec<_> = delays.iter().map(|d| d.unwrap().as_millis()).collect();
        assert_eq!(ms, vec![250, 500, 1000, 2000]);
    }

    #[test]
    fn df_gives_up_after_budget_and_on_non_transient() {
        assert!(disk_usage_retry_delay(4, &server_error(500)).is_none());
        assert!(disk_usage_retry_delay(0, &server_error(404)).is_none());
        let io = std::io::Error::from(std::io::ErrorKind::ConnectionRefused);
        assert!(disk_usage_retry_delay(0, &Error::IOError { err: io }).is_none());
    }

    #[test]
    fn persistent_df_failure_maps_to_failed() {
        assert!(matches!(map_error(server_error(500)), PortError::Failed(_)));
    }

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
    fn volume_prune_filter_includes_named_volumes() {
        let filters = prune_volume_filters();
        assert_eq!(filters.get("all"), Some(&vec!["true".to_owned()]));
    }

    #[test]
    fn connect_to_missing_socket_is_unavailable() {
        let err = DockerEngine::connect("/nonexistent/telima-test.sock").unwrap_err();
        assert!(matches!(err, PortError::Unavailable(_)));
    }
}
