use std::env;

fn main() {
    match env::args().nth(1).as_deref() {
        Some("handshake") => print_handshake(),
        Some("run-provider") => print_provider_run(),
        _ => {
            eprintln!("expected handshake or run-provider");
            std::process::exit(2);
        }
    }
}

fn print_handshake() {
    println!(
        r#"{{"schema_version":"polint-extension-handshake-v1","extension_id":"type_precision","activation_status":"handshake_ok","providers":[{{"provider_id":"aliases","declared_inputs":["source_files"],"declared_outputs":["type_value_alias.alias_answer"]}}],"diagnostics":[]}}"#
    );
}

fn print_provider_run() {
    println!(
        r#"{{"schema_version":"polint-extension-provider-run-v1","extension_id":"type_precision","provider_id":"aliases","activation_status":"active","diagnostics":[],"facts":[{{"fact_family":"type_value_alias.alias_answer","stable_key":"alias:extension:no_alias","binding_refs":["file:src/app.ts"],"precision":"heuristic","confidence":"medium","evidence":["fixture-extension-precision"],"payload_labels":["left=place:1","right=place:2","status=no_alias"]}},{{"fact_family":"type_value_alias.alias_answer","stable_key":"alias:extension:may_alias","binding_refs":["file:src/app.ts"],"precision":"heuristic","confidence":"medium","evidence":["fixture-extension-precision"],"payload_labels":["left=place:1","right=place:2","status=may_alias"]}},{{"fact_family":"type_value_alias.alias_answer","stable_key":"alias:extension:must_alias","binding_refs":["file:src/app.ts"],"precision":"heuristic","confidence":"medium","evidence":["fixture-extension-precision"],"payload_labels":["left=place:1","right=place:1","status=must_alias"]}},{{"fact_family":"type_value_alias.alias_answer","stable_key":"alias:extension:partial_alias","binding_refs":["file:src/app.ts"],"precision":"heuristic","confidence":"medium","evidence":["fixture-extension-precision"],"payload_labels":["left=place:1","right=place:2","status=partial_alias"]}},{{"fact_family":"type_value_alias.alias_answer","stable_key":"alias:extension:unknown","binding_refs":["file:src/app.ts"],"precision":"heuristic","confidence":"medium","evidence":["fixture-extension-precision"],"payload_labels":["left=place:1","right=place:2","status=unknown"]}}],"output_digest_inputs":["fixture=type-value-alias-extension-precision"]}}"#
    );
}
