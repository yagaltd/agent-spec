mod brief;
mod cache;
mod lifecycle;
pub mod plan;

#[allow(deprecated)]
pub use brief::SpecBrief;
pub use brief::TaskContract;
pub use cache::{CacheInputs, CacheStatus, CachedRun};
pub use lifecycle::SpecGateway;
pub use lifecycle::{apply_dependency_skips, topological_sort_scenarios};
