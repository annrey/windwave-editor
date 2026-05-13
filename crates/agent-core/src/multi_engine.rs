//! Multi-Engine Adapter skeletons — Unity, Unreal, Godot
//!
//! Each adapter implements the `EngineAdapterLike` trait so DirectorRuntime
//! can dispatch commands to any supported engine through a uniform interface.
//! These are skeletons ready for protocol-level integration.

use serde::{Deserialize, Serialize};

/// Enumeration of supported game engines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EngineTarget {
    Bevy,
    Unity,
    Unreal,
    Godot,
}

impl EngineTarget {
    pub fn label(&self) -> &str {
        match self {
            EngineTarget::Bevy => "Bevy",
            EngineTarget::Unity => "Unity",
            EngineTarget::Unreal => "Unreal Engine",
            EngineTarget::Godot => "Godot",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Command {
    pub method: String,
    pub params: serde_json::Value,
}

impl Command {
    pub fn new(method: impl Into<String>) -> Self {
        Self {
            method: method.into(),
            params: serde_json::Value::Null,
        }
    }

    pub fn with_params(mut self, params: serde_json::Value) -> Self {
        self.params = params;
        self
    }
}

#[derive(Debug, Serialize)]
struct JsonRpcRequest {
    jsonrpc: &'static str,
    method: String,
    params: serde_json::Value,
    id: u64,
}

#[derive(Debug, Deserialize)]
struct JsonRpcResponse {
    result: Option<serde_json::Value>,
    error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcError {
    message: String,
}

// ---------------------------------------------------------------------------
// Unity Adapter skeleton
// ---------------------------------------------------------------------------

/// Placeholder for Unity Editor integration.
///
/// In production this would communicate with Unity via:
/// - Unity Editor Coroutines + HTTP listener
/// - C# <-> Rust FFI bridge
/// - Shared protobuf / JSON-RPC protocol
#[derive(Debug, Clone, Default)]
pub struct UnityAdapter {
    pub connected: bool,
    pub editor_port: u16,
    #[cfg(feature = "engine-http")]
    http_client: Option<reqwest::Client>,
}

impl UnityAdapter {
    pub fn new(port: u16) -> Self {
        Self {
            connected: false,
            editor_port: port,
            #[cfg(feature = "engine-http")]
            http_client: None,
        }
    }

    #[cfg(feature = "engine-http")]
    pub fn with_http(port: u16) -> Result<Self, String> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
        Ok(Self {
            connected: false,
            editor_port: port,
            http_client: Some(client),
        })
    }

    pub fn connect(&mut self) -> Result<(), String> {
        #[cfg(feature = "engine-http")]
        {
            if let Some(ref client) = self.http_client {
                let url = format!("http://localhost:{}/health", self.editor_port);
                let rt = tokio::runtime::Runtime::new()
                    .map_err(|e| format!("Failed to create runtime: {}", e))?;
                let result = rt.block_on(async { client.get(&url).send().await });
                match result {
                    Ok(response) => {
                        if response.status().is_success() {
                            self.connected = true;
                            return Ok(());
                        } else {
                            return Err(format!(
                                "Unity Editor health check failed: HTTP {}",
                                response.status()
                            ));
                        }
                    }
                    Err(e) => {
                        return Err(format!(
                            "Failed to connect to Unity Editor on port {}: {}",
                            self.editor_port, e
                        ));
                    }
                }
            }
        }
        self.connected = true;
        Ok(())
    }

    pub fn disconnect(&mut self) {
        self.connected = false;
    }

    pub fn is_connected(&self) -> bool {
        self.connected
    }

    pub fn send_command(&self, command: &str) -> Result<String, String> {
        if !self.connected {
            return Err("Unity adapter not connected".into());
        }

        #[cfg(feature = "engine-http")]
        {
            if let Some(ref client) = self.http_client {
                let rpc = JsonRpcRequest {
                    jsonrpc: "2.0",
                    method: command.to_string(),
                    params: serde_json::Value::Null,
                    id: 1,
                };
                let url = format!("http://localhost:{}/rpc", self.editor_port);
                let rt = tokio::runtime::Runtime::new()
                    .map_err(|e| format!("Failed to create runtime: {}", e))?;
                let result = rt.block_on(async { client.post(&url).json(&rpc).send().await });
                match result {
                    Ok(response) => {
                        if !response.status().is_success() {
                            let status = response.status();
                            let body = rt.block_on(async { response.text().await }).unwrap_or_default();
                            return Err(format!("Unity RPC error: HTTP {} - {}", status, body));
                        }
                        let rpc_response: JsonRpcResponse = rt
                            .block_on(async { response.json().await })
                            .map_err(|e| format!("Invalid RPC response: {}", e))?;
                        if let Some(error) = rpc_response.error {
                            return Err(format!("Unity RPC error: {}", error.message));
                        }
                        return Ok(rpc_response
                            .result
                            .map(|v| v.to_string())
                            .unwrap_or_default());
                    }
                    Err(e) => {
                        return Err(format!("Failed to send command to Unity: {}", e));
                    }
                }
            }
        }

        Ok("{}".into())
    }

    pub fn send_jsonrpc(&self, command: &Command) -> Result<String, String> {
        if !self.connected {
            return Err("Unity adapter not connected".into());
        }

        #[cfg(feature = "engine-http")]
        {
            if let Some(ref client) = self.http_client {
                let rpc = JsonRpcRequest {
                    jsonrpc: "2.0",
                    method: command.method.clone(),
                    params: command.params.clone(),
                    id: 1,
                };
                let url = format!("http://localhost:{}/rpc", self.editor_port);
                let rt = tokio::runtime::Runtime::new()
                    .map_err(|e| format!("Failed to create runtime: {}", e))?;
                let result = rt.block_on(async { client.post(&url).json(&rpc).send().await });
                match result {
                    Ok(response) => {
                        if !response.status().is_success() {
                            let status = response.status();
                            let body = rt.block_on(async { response.text().await }).unwrap_or_default();
                            return Err(format!("Unity RPC error: HTTP {} - {}", status, body));
                        }
                        let rpc_response: JsonRpcResponse = rt
                            .block_on(async { response.json().await })
                            .map_err(|e| format!("Invalid RPC response: {}", e))?;
                        if let Some(error) = rpc_response.error {
                            return Err(format!("Unity RPC error: {}", error.message));
                        }
                        return Ok(rpc_response
                            .result
                            .map(|v| v.to_string())
                            .unwrap_or_default());
                    }
                    Err(e) => {
                        return Err(format!("Failed to send command to Unity: {}", e));
                    }
                }
            }
        }

        Ok("{}".into())
    }
}

// ---------------------------------------------------------------------------
// Unreal Engine Adapter skeleton
// ---------------------------------------------------------------------------

/// Placeholder for Unreal Engine integration.
///
/// Production path:
/// - Unreal Editor plugin with HTTP endpoint
/// - Blueprint / C++ bridge for ECS-like operations
/// - JSON-based scene description protocol
#[derive(Debug, Clone, Default)]
pub struct UnrealAdapter {
    pub connected: bool,
    pub editor_port: u16,
    #[cfg(feature = "engine-http")]
    http_client: Option<reqwest::Client>,
}

impl UnrealAdapter {
    pub fn new(port: u16) -> Self {
        Self {
            connected: false,
            editor_port: port,
            #[cfg(feature = "engine-http")]
            http_client: None,
        }
    }

    #[cfg(feature = "engine-http")]
    pub fn with_http(port: u16) -> Result<Self, String> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
        Ok(Self {
            connected: false,
            editor_port: port,
            http_client: Some(client),
        })
    }

    pub fn connect(&mut self) -> Result<(), String> {
        #[cfg(feature = "engine-http")]
        {
            if let Some(ref client) = self.http_client {
                let url = format!("http://localhost:{}/health", self.editor_port);
                let rt = tokio::runtime::Runtime::new()
                    .map_err(|e| format!("Failed to create runtime: {}", e))?;
                let result = rt.block_on(async { client.get(&url).send().await });
                match result {
                    Ok(response) => {
                        if response.status().is_success() {
                            self.connected = true;
                            return Ok(());
                        } else {
                            return Err(format!(
                                "Unreal Editor health check failed: HTTP {}",
                                response.status()
                            ));
                        }
                    }
                    Err(e) => {
                        return Err(format!(
                            "Failed to connect to Unreal Editor on port {}: {}",
                            self.editor_port, e
                        ));
                    }
                }
            }
        }
        self.connected = true;
        Ok(())
    }

    pub fn disconnect(&mut self) {
        self.connected = false;
    }

    pub fn is_connected(&self) -> bool {
        self.connected
    }

    pub fn send_command(&self, command: &str) -> Result<String, String> {
        if !self.connected {
            return Err("Unreal adapter not connected".into());
        }

        #[cfg(feature = "engine-http")]
        {
            if let Some(ref client) = self.http_client {
                let rpc = JsonRpcRequest {
                    jsonrpc: "2.0",
                    method: command.to_string(),
                    params: serde_json::Value::Null,
                    id: 1,
                };
                let url = format!("http://localhost:{}/rpc", self.editor_port);
                let rt = tokio::runtime::Runtime::new()
                    .map_err(|e| format!("Failed to create runtime: {}", e))?;
                let result = rt.block_on(async { client.post(&url).json(&rpc).send().await });
                match result {
                    Ok(response) => {
                        if !response.status().is_success() {
                            let status = response.status();
                            let body = rt.block_on(async { response.text().await }).unwrap_or_default();
                            return Err(format!("Unreal RPC error: HTTP {} - {}", status, body));
                        }
                        let rpc_response: JsonRpcResponse = rt
                            .block_on(async { response.json().await })
                            .map_err(|e| format!("Invalid RPC response: {}", e))?;
                        if let Some(error) = rpc_response.error {
                            return Err(format!("Unreal RPC error: {}", error.message));
                        }
                        return Ok(rpc_response
                            .result
                            .map(|v| v.to_string())
                            .unwrap_or_default());
                    }
                    Err(e) => {
                        return Err(format!("Failed to send command to Unreal: {}", e));
                    }
                }
            }
        }

        Ok("{}".into())
    }

    pub fn send_jsonrpc(&self, command: &Command) -> Result<String, String> {
        if !self.connected {
            return Err("Unreal adapter not connected".into());
        }

        #[cfg(feature = "engine-http")]
        {
            if let Some(ref client) = self.http_client {
                let rpc = JsonRpcRequest {
                    jsonrpc: "2.0",
                    method: command.method.clone(),
                    params: command.params.clone(),
                    id: 1,
                };
                let url = format!("http://localhost:{}/rpc", self.editor_port);
                let rt = tokio::runtime::Runtime::new()
                    .map_err(|e| format!("Failed to create runtime: {}", e))?;
                let result = rt.block_on(async { client.post(&url).json(&rpc).send().await });
                match result {
                    Ok(response) => {
                        if !response.status().is_success() {
                            let status = response.status();
                            let body = rt.block_on(async { response.text().await }).unwrap_or_default();
                            return Err(format!("Unreal RPC error: HTTP {} - {}", status, body));
                        }
                        let rpc_response: JsonRpcResponse = rt
                            .block_on(async { response.json().await })
                            .map_err(|e| format!("Invalid RPC response: {}", e))?;
                        if let Some(error) = rpc_response.error {
                            return Err(format!("Unreal RPC error: {}", error.message));
                        }
                        return Ok(rpc_response
                            .result
                            .map(|v| v.to_string())
                            .unwrap_or_default());
                    }
                    Err(e) => {
                        return Err(format!("Failed to send command to Unreal: {}", e));
                    }
                }
            }
        }

        Ok("{}".into())
    }
}

// ---------------------------------------------------------------------------
// Godot Adapter skeleton
// ---------------------------------------------------------------------------

/// Placeholder for Godot Editor integration.
///
/// Production path:
/// - Godot Editor plugin with TCP listener
/// - GDScript / C# bridge
/// - Scene tree ↔ JSON mapping
#[derive(Debug, Clone, Default)]
pub struct GodotAdapter {
    pub connected: bool,
    pub editor_port: u16,
    #[cfg(feature = "engine-http")]
    http_client: Option<reqwest::Client>,
}

impl GodotAdapter {
    pub fn new(port: u16) -> Self {
        Self {
            connected: false,
            editor_port: port,
            #[cfg(feature = "engine-http")]
            http_client: None,
        }
    }

    #[cfg(feature = "engine-http")]
    pub fn with_http(port: u16) -> Result<Self, String> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
        Ok(Self {
            connected: false,
            editor_port: port,
            http_client: Some(client),
        })
    }

    pub fn connect(&mut self) -> Result<(), String> {
        #[cfg(feature = "engine-http")]
        {
            if let Some(ref client) = self.http_client {
                let url = format!("http://localhost:{}/health", self.editor_port);
                let rt = tokio::runtime::Runtime::new()
                    .map_err(|e| format!("Failed to create runtime: {}", e))?;
                let result = rt.block_on(async { client.get(&url).send().await });
                match result {
                    Ok(response) => {
                        if response.status().is_success() {
                            self.connected = true;
                            return Ok(());
                        } else {
                            return Err(format!(
                                "Godot Editor health check failed: HTTP {}",
                                response.status()
                            ));
                        }
                    }
                    Err(e) => {
                        return Err(format!(
                            "Failed to connect to Godot Editor on port {}: {}",
                            self.editor_port, e
                        ));
                    }
                }
            }
        }
        self.connected = true;
        Ok(())
    }

    pub fn disconnect(&mut self) {
        self.connected = false;
    }

    pub fn is_connected(&self) -> bool {
        self.connected
    }

    pub fn send_command(&self, command: &str) -> Result<String, String> {
        if !self.connected {
            return Err("Godot adapter not connected".into());
        }

        #[cfg(feature = "engine-http")]
        {
            if let Some(ref client) = self.http_client {
                let rpc = JsonRpcRequest {
                    jsonrpc: "2.0",
                    method: command.to_string(),
                    params: serde_json::Value::Null,
                    id: 1,
                };
                let url = format!("http://localhost:{}/rpc", self.editor_port);
                let rt = tokio::runtime::Runtime::new()
                    .map_err(|e| format!("Failed to create runtime: {}", e))?;
                let result = rt.block_on(async { client.post(&url).json(&rpc).send().await });
                match result {
                    Ok(response) => {
                        if !response.status().is_success() {
                            let status = response.status();
                            let body = rt.block_on(async { response.text().await }).unwrap_or_default();
                            return Err(format!("Godot RPC error: HTTP {} - {}", status, body));
                        }
                        let rpc_response: JsonRpcResponse = rt
                            .block_on(async { response.json().await })
                            .map_err(|e| format!("Invalid RPC response: {}", e))?;
                        if let Some(error) = rpc_response.error {
                            return Err(format!("Godot RPC error: {}", error.message));
                        }
                        return Ok(rpc_response
                            .result
                            .map(|v| v.to_string())
                            .unwrap_or_default());
                    }
                    Err(e) => {
                        return Err(format!("Failed to send command to Godot: {}", e));
                    }
                }
            }
        }

        Ok("{}".into())
    }

    pub fn send_jsonrpc(&self, command: &Command) -> Result<String, String> {
        if !self.connected {
            return Err("Godot adapter not connected".into());
        }

        #[cfg(feature = "engine-http")]
        {
            if let Some(ref client) = self.http_client {
                let rpc = JsonRpcRequest {
                    jsonrpc: "2.0",
                    method: command.method.clone(),
                    params: command.params.clone(),
                    id: 1,
                };
                let url = format!("http://localhost:{}/rpc", self.editor_port);
                let rt = tokio::runtime::Runtime::new()
                    .map_err(|e| format!("Failed to create runtime: {}", e))?;
                let result = rt.block_on(async { client.post(&url).json(&rpc).send().await });
                match result {
                    Ok(response) => {
                        if !response.status().is_success() {
                            let status = response.status();
                            let body = rt.block_on(async { response.text().await }).unwrap_or_default();
                            return Err(format!("Godot RPC error: HTTP {} - {}", status, body));
                        }
                        let rpc_response: JsonRpcResponse = rt
                            .block_on(async { response.json().await })
                            .map_err(|e| format!("Invalid RPC response: {}", e))?;
                        if let Some(error) = rpc_response.error {
                            return Err(format!("Godot RPC error: {}", error.message));
                        }
                        return Ok(rpc_response
                            .result
                            .map(|v| v.to_string())
                            .unwrap_or_default());
                    }
                    Err(e) => {
                        return Err(format!("Failed to send command to Godot: {}", e));
                    }
                }
            }
        }

        Ok("{}".into())
    }
}

// ---------------------------------------------------------------------------
// EngineAdapterLike trait — uniform dispatch interface
// ---------------------------------------------------------------------------

/// Minimal trait that every engine adapter must satisfy so DirectorRuntime
/// can treat them uniformly (independent of the full `EngineAdapter` trait
/// which is defined in the Bevy adapter crate).
pub trait EngineAdapterLike: Send + Sync {
    fn engine_target(&self) -> EngineTarget;
    fn is_connected(&self) -> bool;
    fn send_raw_command(&self, command: &str) -> Result<String, String>;
}

impl EngineAdapterLike for UnityAdapter {
    fn engine_target(&self) -> EngineTarget {
        EngineTarget::Unity
    }

    fn is_connected(&self) -> bool {
        self.connected
    }

    fn send_raw_command(&self, command: &str) -> Result<String, String> {
        self.send_command(command)
    }
}

impl EngineAdapterLike for UnrealAdapter {
    fn engine_target(&self) -> EngineTarget {
        EngineTarget::Unreal
    }

    fn is_connected(&self) -> bool {
        self.connected
    }

    fn send_raw_command(&self, command: &str) -> Result<String, String> {
        self.send_command(command)
    }
}

impl EngineAdapterLike for GodotAdapter {
    fn engine_target(&self) -> EngineTarget {
        EngineTarget::Godot
    }

    fn is_connected(&self) -> bool {
        self.connected
    }

    fn send_raw_command(&self, command: &str) -> Result<String, String> {
        self.send_command(command)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_target_labels() {
        assert_eq!(EngineTarget::Bevy.label(), "Bevy");
        assert_eq!(EngineTarget::Unity.label(), "Unity");
        assert_eq!(EngineTarget::Unreal.label(), "Unreal Engine");
        assert_eq!(EngineTarget::Godot.label(), "Godot");
    }

    #[test]
    fn test_unity_adapter_default_disconnected() {
        let adapter = UnityAdapter::default();
        assert!(!adapter.is_connected());
        assert_eq!(adapter.editor_port, 0);

        assert!(adapter.send_command("spawn").is_err());
    }

    #[test]
    fn test_unity_adapter_connect_and_send() {
        let mut adapter = UnityAdapter::new(5500);
        adapter.connect().unwrap();
        assert!(adapter.is_connected());
        assert!(adapter.send_command("spawn").is_ok());
        adapter.disconnect();
        assert!(!adapter.is_connected());
    }

    #[test]
    fn test_unreal_adapter_lifecycle() {
        let mut adapter = UnrealAdapter::new(5501);
        assert!(!adapter.is_connected());
        adapter.connect().unwrap();
        assert!(adapter.is_connected());
        assert!(adapter.send_command("create_actor").is_ok());
    }

    #[test]
    fn test_godot_adapter_lifecycle() {
        let mut adapter = GodotAdapter::new(5502);
        adapter.connect().unwrap();
        assert!(adapter.is_connected());
        adapter.disconnect();
        assert!(!adapter.is_connected());
        assert!(adapter.send_command("add_node").is_err());
    }

    #[test]
    fn test_engine_adapter_like_trait() {
        let mut unity = UnityAdapter::new(5500);
        unity.connect().unwrap();
        let dyn_adapter: &dyn EngineAdapterLike = &unity;
        assert_eq!(dyn_adapter.engine_target(), EngineTarget::Unity);
        assert!(dyn_adapter.is_connected());
        assert!(dyn_adapter.send_raw_command("spawn").is_ok());
    }

    #[test]
    fn test_engine_target_serialization() {
        let target = EngineTarget::Unreal;
        let json = serde_json::to_string(&target).unwrap();
        assert!(json.contains("Unreal"));
        let back: EngineTarget = serde_json::from_str(&json).unwrap();
        assert_eq!(back, EngineTarget::Unreal);
    }

    #[test]
    fn test_command_new() {
        let cmd = Command::new("spawn_entity");
        assert_eq!(cmd.method, "spawn_entity");
        assert!(cmd.params.is_null());
    }

    #[test]
    fn test_command_with_params() {
        let params = serde_json::json!({"name": "Player", "x": 1.0});
        let cmd = Command::new("spawn_entity").with_params(params.clone());
        assert_eq!(cmd.method, "spawn_entity");
        assert_eq!(cmd.params, params);
    }

    #[test]
    fn test_command_serialization() {
        let cmd = Command::new("create_entity").with_params(serde_json::json!({"name": "Enemy"}));
        let json = serde_json::to_string(&cmd).unwrap();
        let back: Command = serde_json::from_str(&json).unwrap();
        assert_eq!(back.method, "create_entity");
        assert_eq!(back.params["name"], "Enemy");
    }

    #[test]
    fn test_unity_send_jsonrpc() {
        let mut adapter = UnityAdapter::new(5500);
        adapter.connect().unwrap();
        let cmd = Command::new("spawn").with_params(serde_json::json!({"type": "cube"}));
        let result = adapter.send_jsonrpc(&cmd);
        assert!(result.is_ok());
    }

    #[test]
    fn test_unreal_send_jsonrpc_disconnected() {
        let adapter = UnrealAdapter::new(5501);
        let cmd = Command::new("create_actor");
        let result = adapter.send_jsonrpc(&cmd);
        assert!(result.is_err());
    }

    #[test]
    fn test_godot_send_jsonrpc() {
        let mut adapter = GodotAdapter::new(5502);
        adapter.connect().unwrap();
        let cmd = Command::new("add_node").with_params(serde_json::json!({"path": "/root/Node3D"}));
        let result = adapter.send_jsonrpc(&cmd);
        assert!(result.is_ok());
    }
}
