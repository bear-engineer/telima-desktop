//! `DockerEngine`: the container-engine port over the Docker Engine API (bollard).

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
        let err = DockerEngine::connect("/nonexistent/telima-test.sock")
            .err()
            .expect("must fail");
        assert!(matches!(err, PortError::Unavailable(_)));
    }
}
