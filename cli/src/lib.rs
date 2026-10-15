//! Manual AWS request construction for the `awsw` inventory tool.
//!
//! This crate intentionally has no dependency on an AWS SDK, AWS CLI, or
//! botocore. General-purpose TLS, serialization, and cryptographic crates are
//! used so that this code owns AWS payload construction without attempting to
//! reinvent TLS or cryptographic primitives.

pub mod client;
pub mod credentials;
pub mod error;
pub mod protocols;
pub mod services;
pub mod sigv4;

pub use client::{AwsClient, Request};
pub use credentials::{CredentialProvider, Credentials, EnvironmentProvider};
pub use error::{Error, Result};
