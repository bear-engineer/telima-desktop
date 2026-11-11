//! Domain layer: entities, value objects, domain rules and port traits.
//! Must not depend on GPUI, tokio, bollard or any adapter (see scripts/check_layers.py).

pub mod container;
pub mod ports;
pub mod profile;
pub mod rules;
