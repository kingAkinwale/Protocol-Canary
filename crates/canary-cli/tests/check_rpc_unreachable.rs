mod support;

use std::net::TcpListener;

use support::{run_in, stdout, TempProject};

const RPC_ONLY_CONFIG: &str = r#"
version = 1
protocol = 28

[tests]
xdr = false
rpc = true
soroban = false
"#;

const RPC_FIXTURE: &str = r#"
id = "p28-rpc-get-network"
protocol = 28
surface = "rpc"
category = "network"
description = "getNetwork reports protocol 28"
method = "get-network"
"#;

/// Binds an ephemeral loopback port and releases it, so the URL points at a
/// port that nothing is listening on.
fn unreachable_rpc_url() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind an ephemeral loopback port");
    let port = listener.local_addr().expect("local address").port();
    drop(listener);
    format!("http://127.0.0.1:{port}")
}

fn rpc_unreachable_project(prefix: &str) -> TempProject {
    let dir = TempProject::new(prefix);
    dir.write(".stellar-canary.toml", RPC_ONLY_CONFIG);
    dir.write("fixtures/p28-rpc-get-network.toml", RPC_FIXTURE);
    dir
}

fn mentions_transport_failure(diagnostic: &str) -> bool {
    let lowered = diagnostic.to_lowercase();
    lowered.contains("transport error") || lowered.contains("connection refused")
}

#[test]
fn an_unreachable_rpc_endpoint_exits_three_in_terminal_output() {
    let dir = rpc_unreachable_project("check-rpc-unreachable-terminal");
    let url = unreachable_rpc_url();

    let output = run_in(&dir.path, &["check", "--rpc-url", &url]);
    assert_eq!(output.status.code(), Some(3));
    let text = stdout(&output);
    assert!(
        text.contains("Network: testnet (protocol not observed)"),
        "stdout: {text}"
    );
    assert!(text.contains("Status: ERROR"), "stdout: {text}");
    assert!(text.contains("Failure:"), "stdout: {text}");
    assert!(text.contains("p28-rpc-get-network"), "stdout: {text}");
    assert!(
        mentions_transport_failure(&text),
        "expected a transport-failure diagnostic in stdout: {text}"
    );
}

#[test]
fn an_unreachable_rpc_endpoint_produces_a_cf01_error_report_in_json() {
    let dir = rpc_unreachable_project("check-rpc-unreachable-json");
    let url = unreachable_rpc_url();

    let output = run_in(&dir.path, &["check", "--json", "--rpc-url", &url]);
    assert_eq!(output.status.code(), Some(3));

    let value: serde_json::Value = serde_json::from_str(&stdout(&output))
        .expect("stdout must be a valid CF-01 JSON report");
    assert_eq!(value["schemaVersion"], 1);
    assert_eq!(value["targetProtocol"], 28);
    assert_eq!(value["status"], "error");
    assert_eq!(value["counts"]["errors"], 1);
    assert_eq!(value["counts"]["passed"], 0);

    let results = value["results"]
        .as_array()
        .expect("CF-01 results must be an array");
    assert_eq!(results.len(), 1);
    let result = &results[0];
    assert_eq!(result["testId"], "p28-rpc-get-network");
    assert_eq!(result["status"], "error");
    let diagnostic = format!("{} {}", result["summary"], result["details"]);
    assert!(
        mentions_transport_failure(&diagnostic),
        "expected a transport-failure diagnostic in the fixture result: {diagnostic}"
    );
}
