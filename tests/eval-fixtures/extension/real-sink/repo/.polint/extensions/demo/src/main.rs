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
        r#"{{"schema_version":"polint-extension-handshake-v1","extension_id":"demo","activation_status":"handshake_ok","providers":[{{"provider_id":"routes","declared_inputs":["source_files"],"declared_outputs":["extension.routes"]}}],"diagnostics":[]}}"#
    );
}

fn print_provider_run() {
    println!(
        r#"{{"schema_version":"polint-extension-provider-run-v1","extension_id":"demo","provider_id":"routes","activation_status":"active","diagnostics":[],"facts":[{{"fact_family":"extension.routes","stable_key":"extension.route./ok","binding_refs":["file:src/app.ts"],"precision":"heuristic","confidence":"medium","evidence":["fixture-real-sink"],"payload_labels":["kind=route","path=/ok"]}},{{"fact_family":"extension.undeclared","stable_key":"extension.route./rejected","binding_refs":["file:src/app.ts"],"precision":"heuristic","confidence":"medium","evidence":["fixture-real-sink"],"payload_labels":["kind=route","path=/rejected"]}}],"output_digest_inputs":["fixture=real-sink"]}}"#
    );
}
