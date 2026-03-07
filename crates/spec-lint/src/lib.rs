#![warn(clippy::all)]
#![deny(unsafe_code)]

mod linters;
mod pipeline;

pub use linters::*;
pub use pipeline::LintPipeline;
pub use pipeline::cross_check;
