use odekrai_lib::mcp::{
    ClientError, McpStdioClient, StdioConfig, MAX_STDERR_BYTES, MAX_TOOL_RESULT_BYTES,
};
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

fn spawn(mode: &str) -> McpStdioClient {
    let mut env = BTreeMap::new();
    env.insert("ODEKRAI_MCP_FAKE".into(), mode.into());
    env.insert("ODEKRAI_MCP_ALLOWED".into(), "visible".into());
    let mut config = StdioConfig::new(env!("CARGO_BIN_EXE_mcp_fake_server"), Vec::new());
    config.env = env;
    config.connection_timeout = Duration::from_secs(3);
    config.call_timeout = Duration::from_millis(250);
    McpStdioClient::connect(config).unwrap()
}

#[test]
fn handshake_list_call_out_of_order_and_untrusted_output() {
    let client = spawn("normal");
    assert_eq!(client.tools_list().unwrap()["tools"][0]["name"], "echo");
    assert_eq!(
        client.tools_call("echo", serde_json::json!({})).unwrap()["_untrusted"],
        true
    );
    let huge = spawn("huge")
        .tools_call("echo", serde_json::json!({}))
        .unwrap();
    assert!(huge.to_string().contains("truncado"));
    assert!(serde_json::to_vec(&huge).unwrap().len() <= MAX_TOOL_RESULT_BYTES);
    let stderr = spawn("stderr");
    std::thread::sleep(Duration::from_millis(40));
    assert!(stderr.stderr_tail().len() <= MAX_STDERR_BYTES);
}

#[test]
fn timeout_cancel_invalid_json_crash_and_restart() {
    assert!(matches!(
        spawn("hang").tools_call("wait", serde_json::json!({})),
        Err(ClientError::Timeout)
    ));
    let cancelled = spawn("cancel");
    let token = Arc::new(AtomicBool::new(false));
    let trigger = token.clone();
    let cancel_after_request = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(50));
        trigger.store(true, Ordering::Relaxed);
    });
    assert!(matches!(
        cancelled.tools_call_cancellable(
            "wait",
            serde_json::json!({}),
            Duration::from_secs(2),
            &token
        ),
        Err(ClientError::Cancelled)
    ));
    cancel_after_request.join().unwrap();
    let capture_deadline = std::time::Instant::now() + Duration::from_secs(1);
    while !String::from_utf8_lossy(&cancelled.stderr_tail()).contains("cancel-received")
        && std::time::Instant::now() < capture_deadline
    {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(String::from_utf8_lossy(&cancelled.stderr_tail()).contains("cancel-received"));
    assert!(matches!(
        spawn("invalid").tools_call("bad", serde_json::json!({})),
        Err(ClientError::Protocol(_))
    ));
    assert!(matches!(
        spawn("oversize").tools_call("huge", serde_json::json!({})),
        Err(ClientError::Protocol(_))
    ));
    assert!(matches!(
        spawn("crash").tools_list(),
        Err(ClientError::Disconnected)
    ));
    let crash_during_call = spawn("crash_call");
    assert_eq!(crash_during_call.tools_list().unwrap()["tools"][0]["name"], "echo");
    assert!(matches!(
        crash_during_call.tools_call("echo", serde_json::json!({})),
        Err(ClientError::Disconnected)
    ));
    assert_eq!(
        spawn("normal").tools_list().unwrap()["tools"][0]["name"],
        "echo"
    );
}
