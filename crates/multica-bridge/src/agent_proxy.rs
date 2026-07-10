use crate::error::Result;
use crate::types::*;
use crate::ws_client::MulticaWebSocketClient;

/// Agent proxy for WindWave
pub struct AgentProxy {
    ws_client: MulticaWebSocketClient,
    agent_info: AgentInfo,
}

/// Local agent info
#[derive(Debug, Clone)]
pub struct AgentInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub provider: AgentProvider,
}

impl AgentProxy {
    /// Create new agent proxy
    pub fn new(config: BridgeConfig, name: String) -> Self {
        Self {
            ws_client: MulticaWebSocketClient::new(config),
            agent_info: AgentInfo {
                id: uuid::Uuid::new_v4().to_string(),
                name,
                description: "WindWave Game Editor Agent".to_string(),
                provider: AgentProvider::WindWave,
            },
        }
    }

    /// Connect and register
    pub async fn connect(&mut self) -> Result<()> {
        self.ws_client.connect().await?;
        self.ws_client.register_daemon().await?;
        Ok(())
    }

    /// Get agent info
    pub fn agent_info(&self) -> &AgentInfo {
        &self.agent_info
    }

    /// Check if connected
    pub fn is_connected(&self) -> bool {
        self.ws_client.is_connected()
    }

    /// Receive messages
    pub async fn receive_message(&mut self) -> Result<Option<Message>> {
        self.ws_client.receive_message().await
    }

    /// Disconnect
    pub async fn disconnect(&mut self) -> Result<()> {
        self.ws_client.disconnect().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_info_creation() {
        let info = AgentInfo {
            id: "agent-1".into(),
            name: "TestAgent".into(),
            description: "Test Description".into(),
            provider: AgentProvider::WindWave,
        };
        assert_eq!(info.id, "agent-1");
        assert_eq!(info.name, "TestAgent");
    }

    #[test]
    fn test_agent_proxy_new() {
        let config = BridgeConfig::default();
        let proxy = AgentProxy::new(config, "TestAgent".into());
        assert_eq!(proxy.agent_info().name, "TestAgent");
        assert!(!proxy.is_connected());
    }
}
