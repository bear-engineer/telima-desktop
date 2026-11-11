//! Application layer: use cases, state store and view models. Pure and synchronous to test.
//! Depends only on `telima-core`.

pub mod engine;
pub mod error;
pub mod profiles;

#[cfg(any(test, feature = "testing"))]
pub mod testing;

pub use error::AppError;
