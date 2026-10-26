//! `quarry-mcp`: an MCP stdio server whose tools are the command registry.
//!
//! Each tool call goes over the local socket to the running Quarry window, so
//! the agent edits the same document and history as the human. If no Quarry is
//! running, a headless one is started next to this binary.

use quarry_core::CommandRegistry;
use quarry_server::{Client, SocketAddr};
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, ErrorData,
    Implementation, InitializeResult, ListResourcesResult, ListToolsResult, PaginatedRequestParams,
    ReadResourceRequestParams, ReadResourceResponse, ReadResourceResult, Resource,
    ResourceContents, ServerCapabilities, Tool,
};
use rmcp::service::{RequestContext, RoleServer};
use rmcp::{ServerHandler, ServiceExt};
use serde_json::{Map, Value};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const COMMANDS_URI: &str = "quarry://commands";
const GUIDE_URI: &str = "quarry://guide";
const GUIDE: &str = include_str!("guide.md");

/// `mesh.extrude` becomes the tool `mesh_extrude`, and back.
pub fn tool_name(command: &str) -> String {
    command.replacen('.', "_", 1)
}

/// Every registry command as an MCP tool, generated, never hand-written.
pub fn tools() -> Vec<Tool> {
    CommandRegistry::new()
        .commands()
        .map(|c| {
            let schema = match &c.schema {
                Value::Object(map) => map.clone(),
                _ => Map::new(),
            };
            Tool::new(tool_name(c.name), c.description, Arc::new(schema))
        })
        .collect()
}

fn command_name(tool: &str) -> Option<&'static str> {
    CommandRegistry::new()
        .commands()
        .find(|c| tool_name(c.name) == tool)
        .map(|c| c.name)
}

#[derive(Clone)]
struct Bridge {
    addr: SocketAddr,
    client: Arc<Mutex<Option<Client>>>,
}

impl Bridge {
    /// Connect to the open Quarry, starting a headless one if none answers.
    fn client(&self) -> Result<std::sync::MutexGuard<'_, Option<Client>>, String> {
        let mut guard = self
            .client
            .lock()
            .map_err(|_| "the bridge state is poisoned")?;
        if guard.is_none() {
            let client = match Client::connect(&self.addr) {
                Ok(c) => c,
                Err(_) => {
                    start_headless(&self.addr)?;
                    (0..100)
                        .find_map(|_| {
                            std::thread::sleep(Duration::from_millis(50));
                            Client::connect(&self.addr).ok()
                        })
                        .ok_or("Started a headless Quarry but could not reach it.")?
                }
            };
            *guard = Some(client);
        }
        Ok(guard)
    }

    fn call(&self, command: &str, args: Value) -> Result<Value, String> {
        let mut guard = self.client()?;
        let result = guard
            .as_mut()
            .expect("connected above")
            .call(command, args, &mut |_| {});
        result.map_err(|e| {
            *guard = None; // reconnect next time
            format!("Lost the connection to Quarry ({e}). Try again.")
        })
    }
}

fn start_headless(addr: &SocketAddr) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let quarry = exe.with_file_name(if cfg!(windows) {
        "quarry.exe"
    } else {
        "quarry"
    });
    let dir = std::env::current_dir().map_err(|e| e.to_string())?;
    std::process::Command::new(&quarry)
        .args(["--headless", "--socket", &addr.0, "--dir"])
        .arg(dir)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .spawn()
        .map(drop)
        .map_err(|e| {
            format!(
                "No Quarry is open and {} could not start: {e}.",
                quarry.display()
            )
        })
}

impl ServerHandler for Bridge {
    fn get_info(&self) -> InitializeResult {
        InitializeResult::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .build(),
        )
        .with_server_info(Implementation::new("quarry", env!("CARGO_PKG_VERSION")))
        .with_instructions(
            "Quarry is a low-poly modeler. Read quarry://guide first. Units are metres, Y is up. Every tool edits the same scene and undo history as the human; take view_screenshot after each major step.",
        )
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(ListToolsResult::with_all_items(tools()))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let Some(command) = command_name(&request.name) else {
            return Err(ErrorData::invalid_params(
                format!("There is no tool called {}.", request.name),
                None,
            ));
        };
        let args = Value::Object(request.arguments.unwrap_or_default());
        let bridge = self.clone();
        let reply = tokio::task::spawn_blocking(move || bridge.call(command, args))
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;
        let result = match reply {
            Ok(mut reply) if reply["ok"] == true => {
                // Screenshots come back as images the model can see, not text.
                let result = &mut reply["result"];
                match result
                    .get("image")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                {
                    Some(data) => {
                        let mime = result["mime"].as_str().unwrap_or("image/png").to_string();
                        if let Value::Object(map) = result {
                            map.remove("image");
                        }
                        CallToolResult::success(vec![
                            ContentBlock::image(data, mime),
                            ContentBlock::text(result.to_string()),
                        ])
                    }
                    None => CallToolResult::success(vec![ContentBlock::text(result.to_string())]),
                }
            }
            Ok(reply) => CallToolResult::error(vec![ContentBlock::text(
                reply["error"]["message"]
                    .as_str()
                    .unwrap_or("Quarry refused the call.")
                    .to_string(),
            )]),
            Err(why) => CallToolResult::error(vec![ContentBlock::text(why)]),
        };
        Ok(result.into())
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, ErrorData> {
        let guide = Resource::new(GUIDE_URI, "guide").with_description(
            "Read first: units, axes, naming, the workflow and how to recover from errors.",
        );
        let commands = Resource::new(COMMANDS_URI, "commands")
            .with_description("Every Quarry command with a one-line description.");
        Ok(ListResourcesResult::with_all_items(vec![guide, commands]))
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, ErrorData> {
        if request.uri == GUIDE_URI {
            return Ok(
                ReadResourceResult::new(vec![ResourceContents::text(GUIDE, GUIDE_URI)]).into(),
            );
        }
        if request.uri != COMMANDS_URI {
            return Err(ErrorData::resource_not_found(
                format!(
                    "No resource at {}. Try {GUIDE_URI} or {COMMANDS_URI}.",
                    request.uri
                ),
                None,
            ));
        }
        let list: String = CommandRegistry::new()
            .commands()
            .map(|c| format!("{}: {}\n", tool_name(c.name), c.description))
            .collect();
        Ok(ReadResourceResult::new(vec![ResourceContents::text(list, COMMANDS_URI)]).into())
    }
}

#[tokio::main]
async fn main() {
    let addr = match SocketAddr::from_env() {
        Ok(addr) => addr,
        Err(e) => {
            eprintln!("quarry-mcp: could not pick a socket path: {e}");
            std::process::exit(1);
        }
    };
    let bridge = Bridge {
        addr,
        client: Arc::default(),
    };
    let service = match bridge.serve(rmcp::transport::stdio()).await {
        Ok(service) => service,
        Err(e) => {
            eprintln!("quarry-mcp: could not start: {e}");
            std::process::exit(1);
        }
    };
    let _ = service.waiting().await;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FR-MCP-01: every command is a tool and every tool is a command.
    #[test]
    fn every_command_has_a_tool_and_back() {
        let registry = CommandRegistry::new();
        let tools = tools();
        assert_eq!(tools.len(), registry.commands().count());
        for c in registry.commands() {
            let tool = tools
                .iter()
                .find(|t| t.name == tool_name(c.name))
                .unwrap_or_else(|| panic!("{} has no MCP tool", c.name));
            assert_eq!(tool.description.as_deref(), Some(c.description));
            assert!(
                !tool.input_schema.is_empty(),
                "{} tool has a schema",
                c.name
            );
            assert_eq!(command_name(&tool.name), Some(c.name));
        }
    }
}
