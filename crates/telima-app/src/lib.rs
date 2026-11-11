//! Application layer: use cases, state store and view models. Pure and synchronous to test.
//! Depends only on `telima-core`.

pub mod command;
pub mod engine;
pub mod error;
pub mod profiles;
pub mod runtime;
pub mod view;

#[cfg(any(test, feature = "testing"))]
pub mod testing;

pub use command::AppCommand;
pub use error::AppError;
