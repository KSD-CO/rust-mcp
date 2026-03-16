//! Domain layer — business models and port definitions.
//!
//! This layer has ZERO dependencies on infrastructure (reqwest, HTTP)
//! or framework (mcp-kit, clap). It defines only pure data types and
//! the abstract trait (`DatabasePort`) that infrastructure implements.

pub mod model;
pub mod port;
