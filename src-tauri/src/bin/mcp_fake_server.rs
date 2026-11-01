use serde_json::{json, Value};
use std::io::{self, BufRead, Write};

fn main() {
    let mode = std::env::var("ODEKRAI_MCP_FAKE").unwrap_or_default();
    if std::env::var("ODEKRAI_MCP_ALLOWED").as_deref() != Ok("visible")
        || std::env::var("PATH").is_ok()
    {
        std::process::exit(23);
    }
    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();
    let stdout = io::stdout();
    let mut output = stdout.lock();
    let mut index = 0;
    while let Some(line) = lines.next() {
        let Ok(line) = line else { return };
        let Ok(request) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if request["method"] == "notifications/initialized" {
            continue;
        }
        if mode == "stderr" && request["method"] == "initialize" {
            let _ = write!(io::stderr(), "{}", "e".repeat(2 * 16 * 1024));
        }
        if request["method"] == "notifications/cancelled" {
            let _ = writeln!(io::stderr(), "cancel-received");
            continue;
        }
        if mode == "crash" && request["method"] == "tools/list" {
            return;
        }
        if mode == "crash_call" && request["method"] == "tools/call" {
            return;
        }
        if mode == "hang" && request["method"] == "tools/call" {
            std::thread::sleep(std::time::Duration::from_secs(2));
            continue;
        }
        if mode == "cancel" && request["method"] == "tools/call" {
            if let Some(Ok(notification)) = lines.next() {
                if notification.contains("notifications/cancelled") {
                    let _ = writeln!(io::stderr(), "cancel-received");
                }
            }
            continue;
        }
        if mode == "invalid" && request["method"] == "tools/call" {
            let _ = output.write_all(b"{bad json}\n");
            let _ = output.flush();
            return;
        }
        let result = match request["method"].as_str().unwrap_or("") {
            "initialize" => {
                json!({"protocolVersion":"2025-11-25","capabilities":{"tools":{}},"serverInfo":{"name":"fake","version":"1"}})
            }
            "tools/list" => json!({"tools":[{"name":"echo","inputSchema":{"type":"object"}}]}),
            "tools/call" if mode == "huge" => {
                json!({"content":[{"type":"text","text":"x".repeat(64*1024)}],"isError":false})
            }
            "tools/call" if mode == "oversize" => {
                json!({"content":[{"type":"text","text":"x".repeat(2*1024*1024)}],"isError":false})
            }
            "tools/call" => json!({"content":[{"type":"text","text":"ok"}],"isError":false}),
            _ => json!({}),
        };
        if index == 0 {
            let _ = writeln!(output, "{{\"jsonrpc\":\"2.0\",\"id\":999,\"result\":{{}}}}");
        }
        let response = json!({"jsonrpc":"2.0","id":request["id"],"result":result});
        let _ = writeln!(output, "{response}");
        let _ = output.flush();
        index += 1;
    }
}
