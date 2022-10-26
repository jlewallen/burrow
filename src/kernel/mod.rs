use anyhow::Result;
use once_cell::sync::Lazy;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    collections::HashMap,
    fmt::{Debug, Display},
    ops::Index,
    rc::Weak,
};
use thiserror::Error;
use tracing::{span, Level};

pub mod infra;
pub mod model;
pub mod replies;
pub mod scopes;

pub use infra::*;
pub use model::*;
pub use replies::*;
pub use scopes::*;
