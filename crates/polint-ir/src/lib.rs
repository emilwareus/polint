//! Language-neutral MIR: blocks, terminators, places, and type shapes.
//!
//! Depends only on `polint-core`. Frontends and analyses must not be imported here.

mod body;
mod ids;
mod op;
mod places;
mod types;

pub use body::*;
pub use ids::*;
pub use op::*;
pub use places::*;
pub use types::*;
