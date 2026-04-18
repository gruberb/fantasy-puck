//! Infrastructure adapters: the layer where `domain::*` meets the
//! database and external APIs. Handlers depend on `infra`; `domain`
//! never does.

pub mod calibrate;
pub mod prediction;
