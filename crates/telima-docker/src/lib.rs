//! Adapter: implements the container-engine port on the Docker Engine API (bollard).

pub mod engine;
pub mod map;

pub use engine::DockerEngine;
