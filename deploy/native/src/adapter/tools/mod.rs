//! MCP tool adapters — expose domain operations as MCP tools.

mod query;
mod schema;
mod stats;

pub use query::*;
pub use schema::*;
pub use stats::*;
