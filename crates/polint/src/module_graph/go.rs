//! Internal composition re-exports for the Go module-graph adapter.

pub(crate) use polint_go::module_graph::{
    GoPackageIndex, collect_go_topology, resolve_go_import, seed_go_module_nodes,
};
