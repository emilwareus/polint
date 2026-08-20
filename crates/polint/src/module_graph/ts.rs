//! Internal composition re-exports for the TypeScript/JavaScript module-graph adapter.

pub(crate) use crate::ts::module_graph::{
    TsResolverContext, collect_ts_topology, resolve_ts_import, seed_ts_project_module_nodes,
};
