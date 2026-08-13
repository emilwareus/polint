use crate::analysis_api::{Digest, DigestKind};

pub fn cfg_provider_parameter_digest() -> Digest {
    Digest::from_parts(
        DigestKind::ProviderParameters,
        "cfg_provider_parameters",
        &[
            "cfg-facts-1",
            "cfg_functions",
            "cfg_nodes",
            "basic_blocks",
            "cfg_edges",
            "cfg_reachability",
            "cfg_dominators",
            "cfg_postdominators",
            "cfg_control_dependence",
            "unsupported_control_flow",
            "normal_control_view",
            "abrupt_aware_view",
        ],
    )
}
