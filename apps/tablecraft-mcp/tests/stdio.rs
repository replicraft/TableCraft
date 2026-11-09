//! The published binary speaks MCP over real stdio and uses the native creation actor.
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{self, Receiver},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

const RESPONSE_TIMEOUT: Duration = Duration::from_secs(30);
const CREATION_TIMEOUT: Duration = Duration::from_secs(30);
const POLL_INTERVAL: Duration = Duration::from_millis(5);

struct Peer {
    child: Child,
    input: ChildStdin,
    output: Receiver<Result<Value, String>>,
    reader: Option<JoinHandle<()>>,
    next_id: u64,
}

impl Peer {
    fn start(root: &std::path::Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_tablecraft-mcp"))
            .arg("--root")
            .arg(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let input = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (sender, output) = mpsc::channel();
        let reader = thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let message = line.map_err(|error| error.to_string()).and_then(|line| {
                    serde_json::from_str(&line).map_err(|error| error.to_string())
                });
                if sender.send(message).is_err() {
                    return;
                }
            }
            let _ = sender.send(Err("MCP stdout closed before the expected reply".into()));
        });
        Self {
            child,
            input,
            output,
            reader: Some(reader),
            next_id: 0,
        }
    }

    fn send(&mut self, message: Value) {
        serde_json::to_writer(&mut self.input, &message).unwrap();
        self.input.write_all(b"\n").unwrap();
        self.input.flush().unwrap();
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        self.next_id += 1;
        let id = self.next_id;
        self.send(json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        let deadline = Instant::now() + RESPONSE_TIMEOUT;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let mut message = self
                .output
                .recv_timeout(remaining)
                .expect("MCP response timed out; Drop terminates the child")
                .expect("MCP emitted an invalid or absent JSON message");
            if message.get("id") == Some(&json!(id)) {
                assert_eq!(message["jsonrpc"], "2.0");
                assert!(
                    message.get("error").is_none(),
                    "unexpected JSON-RPC failure: {message}"
                );
                return message["result"].take();
            }
            assert!(
                message.get("id").is_none(),
                "unexpected MCP request or response: {message}"
            );
        }
    }

    fn initialize(&mut self) {
        let initialized = self.request(
            "initialize",
            json!({
                "protocolVersion": "2025-03-26",
                "capabilities": {},
                "clientInfo": {"name": "tablecraft-stdio-acceptance", "version": "0.1.0"},
            }),
        );
        assert!(initialized["protocolVersion"].is_string());
        assert!(initialized["capabilities"]["tools"].is_object());
        self.send(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
    }

    fn tool(&mut self, name: &str, arguments: Value) -> Value {
        self.request("tools/call", json!({"name": name, "arguments": arguments}))
    }

    fn successful_tool(&mut self, name: &str, arguments: Value) -> Value {
        let mut result = self.tool(name, arguments);
        assert_ne!(result["isError"], true, "{name} failed: {result}");
        assert!(
            result["structuredContent"].is_object(),
            "{name} has no structured output: {result}"
        );
        result["structuredContent"].take()
    }
}

impl Drop for Peer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

#[test]
fn stdio_initialize_discovery_native_creation_and_scoped_query_contracts() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("authorized");
    std::fs::create_dir(&root).unwrap();
    let mut peer = Peer::start(&root);
    peer.initialize();
    let listed = peer.request("tools/list", json!({}));
    let tools = listed["tools"].as_array().unwrap();
    let rows = tools
        .iter()
        .find(|tool| tool["name"] == "read_rows")
        .unwrap();
    assert_eq!(rows["inputSchema"]["type"], "object");
    assert_eq!(
        rows["inputSchema"]["properties"]["offset"]["type"],
        "integer"
    );
    assert_eq!(rows["inputSchema"]["additionalProperties"], false);
    let apply = tools
        .iter()
        .find(|tool| tool["name"] == "apply_edit")
        .unwrap();
    assert!(apply["inputSchema"]["properties"]["bundle"].is_null());
    for operation in [
        "create_database",
        "creation_status",
        "acknowledge_creation",
        "open_created_database",
        "inspect_query",
        "run_query",
    ] {
        assert!(
            tools.iter().any(|tool| tool["name"] == operation),
            "missing {operation}"
        );
    }

    let wrong_type = peer.tool(
        "read_rows",
        json!({"document_revision": 1, "table": "Items", "offset": "zero"}),
    );
    assert_eq!(wrong_type["isError"], true);
    assert!(
        wrong_type["structuredContent"]["error"]
            .as_str()
            .unwrap()
            .contains("invalid type")
    );
    let destination = root.join("created.accdb");
    let started = peer.successful_tool("create_database", json!({"path": destination}));
    let id = started["creation_id"].as_u64().unwrap();
    let deadline = Instant::now() + CREATION_TIMEOUT;
    loop {
        let status = peer.successful_tool("creation_status", json!({"creation_id": id}));
        if status["status"] != "pending" {
            assert_eq!(status["status"], "created", "{status}");
            assert_eq!(status["table_count"], 0);
            break;
        }
        assert!(Instant::now() < deadline, "native creation actor stalled");
        thread::sleep(POLL_INTERVAL);
    }
    let acknowledged = peer.successful_tool("acknowledge_creation", json!({"creation_id": id}));
    assert_eq!(acknowledged["acknowledged"], true);
    let opened = peer.successful_tool("open_created_database", json!({"creation_id": id}));
    let revision = opened["catalog"]["document_revision"].as_u64().unwrap();
    let original = std::fs::read(&destination).unwrap();
    assert!(!original.is_empty());

    let outside = directory.path().join("outside.accdb");
    std::fs::write(&outside, &original).unwrap();
    let refused = peer.tool("open_database", json!({"path": outside}));
    assert_eq!(refused["isError"], true);
    assert!(
        refused["structuredContent"]["error"]
            .as_str()
            .unwrap()
            .contains("outside")
    );
    let current = peer.successful_tool("list_tables", json!({}));
    assert_eq!(current["catalog"]["document_revision"], revision);

    let inspected = peer.successful_tool(
        "inspect_query",
        json!({
            "document_revision": revision, "sql": "SELECT 1 AS Answer;",
        }),
    );
    let result = peer.successful_tool(
        "run_query",
        json!({
            "document_revision": revision, "query_id": inspected["query_id"],
        }),
    );
    assert_eq!(result["rows"][0][0]["text"], "1");
    assert_eq!(std::fs::read(destination).unwrap(), original);
    assert_eq!(std::fs::read(outside).unwrap(), original);
}
