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
        r#"{{"schema_version":"polint-extension-handshake-v1","extension_id":"demo","activation_status":"handshake_ok","providers":[{{"provider_id":"refined","declared_inputs":["source_files"],"declared_outputs":["refined_calls.edge"]}}],"diagnostics":[]}}"#
    );
}

fn print_provider_run() {
    println!(
        r#"{{"schema_version":"polint-extension-provider-run-v1","extension_id":"demo","provider_id":"refined","activation_status":"active","diagnostics":[],"facts":[{{"fact_family":"refined_calls.edge","stable_key":"extension:refined-ok","binding_refs":["file:src/app.ts"],"precision":"heuristic","confidence":"medium","evidence":["fixture-extension-model"],"payload_labels":["site=file_callee:src/app.ts:modelTarget","synthetic_target=extension:model-target","algorithm=repo_model","status=resolved"]}},{{"fact_family":"refined_calls.edge","stable_key":"extension:refined-rejected","binding_refs":["file:src/app.ts"],"precision":"heuristic","confidence":"medium","evidence":["fixture-extension-model"],"payload_labels":["site=file_callee:src/app.ts:modelTarget","algorithm=repo_model","status=resolved"]}}],"output_digest_inputs":["fixture=refined-calls-extension-model"]}}"#
    );
}
