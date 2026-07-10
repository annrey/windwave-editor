use crate::dspy_integration::DSPyIntegration;
use crate::error::{Error, Result};
use crate::langchain_integration::LangChainIntegration;
use crate::llamaindex_integration::LlamaIndexIntegration;
use crate::types::*;
use log::{debug, info};
use std::collections::HashMap;

#[allow(dead_code)]
pub struct AIOrchestrator {
    config: AIConfig,
    langchain: Option<LangChainIntegration>,
    llamaindex: Option<LlamaIndexIntegration>,
    dspy: Option<DSPyIntegration>,
    agents: HashMap<String, AgentConfig>,
    indexes: HashMap<String, String>,
}

impl AIOrchestrator {
    pub fn new(config: AIConfig) -> Self {
        Self {
            config: config.clone(),
            langchain: Some(LangChainIntegration::new(config.clone())),
            llamaindex: Some(LlamaIndexIntegration::new(config.clone())),
            dspy: Some(DSPyIntegration::new(config)),
            agents: HashMap::new(),
            indexes: HashMap::new(),
        }
    }

    pub async fn chat(
        &self,
        messages: Vec<ChatMessage>,
        framework: FrameworkType,
    ) -> Result<AIResponse> {
        debug!("Chat request using framework: {:?}", framework);

        match framework {
            FrameworkType::LangChain => {
                if let Some(lc) = &self.langchain {
                    lc.chat(messages).await
                } else {
                    Err(Error::LangChainError(
                        "LangChain not initialized".to_string(),
                    ))
                }
            }
            FrameworkType::LlamaIndex => Err(Error::LlamaIndexError(
                "LlamaIndex chat not implemented".to_string(),
            )),
            FrameworkType::DSPy => Err(Error::DSPyError("DSPy chat not implemented".to_string())),
        }
    }

    pub async fn register_agent(&mut self, agent: AgentConfig) -> Result<()> {
        info!("Registering agent: {}", agent.name);
        self.agents.insert(agent.name.clone(), agent);
        Ok(())
    }

    pub async fn execute_agent(&self, agent_name: &str, input: &str) -> Result<AIResponse> {
        debug!("Executing agent: {}", agent_name);

        let agent = self
            .agents
            .get(agent_name)
            .ok_or_else(|| Error::ConfigError(format!("Agent not found: {}", agent_name)))?;

        let messages = vec![
            ChatMessage {
                role: "system".to_string(),
                content: agent.system_prompt.clone(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: input.to_string(),
            },
        ];

        self.chat(messages, agent.framework.clone()).await
    }

    pub async fn create_knowledge_base(
        &mut self,
        name: &str,
        documents: Vec<Document>,
    ) -> Result<String> {
        info!("Creating knowledge base: {}", name);

        if let Some(li) = &self.llamaindex {
            let index_id = li.create_index(documents).await?;
            self.indexes.insert(name.to_string(), index_id.clone());
            Ok(index_id)
        } else {
            Err(Error::LlamaIndexError(
                "LlamaIndex not initialized".to_string(),
            ))
        }
    }

    pub async fn query_knowledge_base(&self, name: &str, query: &str) -> Result<QueryResult> {
        debug!("Querying knowledge base: {}", name);

        let index_id = self
            .indexes
            .get(name)
            .ok_or_else(|| Error::ConfigError(format!("Knowledge base not found: {}", name)))?;

        if let Some(li) = &self.llamaindex {
            li.query_index(query, index_id).await
        } else {
            Err(Error::LlamaIndexError(
                "LlamaIndex not initialized".to_string(),
            ))
        }
    }

    pub async fn create_pipeline(
        &self,
        name: &str,
        prompt: &str,
        examples: Vec<String>,
    ) -> Result<String> {
        info!("Creating DSPy pipeline: {}", name);

        if let Some(dspy) = &self.dspy {
            dspy.compile_pipeline(prompt, examples).await
        } else {
            Err(Error::DSPyError("DSPy not initialized".to_string()))
        }
    }

    pub async fn execute_pipeline(&self, pipeline_id: &str, input: &str) -> Result<AIResponse> {
        debug!("Executing DSPy pipeline: {}", pipeline_id);

        if let Some(dspy) = &self.dspy {
            dspy.execute_pipeline(input, pipeline_id).await
        } else {
            Err(Error::DSPyError("DSPy not initialized".to_string()))
        }
    }

    pub async fn execute_workflow(&self, steps: Vec<WorkflowStep>) -> Result<WorkflowResult> {
        info!("Executing workflow with {} steps", steps.len());

        let mut step_results = Vec::new();
        let mut outputs: HashMap<String, String> = HashMap::new();

        for step in steps {
            debug!("Processing step: {}", step.name);

            let mut input = step.input.clone();
            for dep in &step.dependencies {
                if let Some(output) = outputs.get(dep) {
                    input = input.replace(&format!("{{{}}}", dep), output);
                }
            }

            let result = self.execute_agent(&step.agent, &input).await;

            match result {
                Ok(response) => {
                    step_results.push(StepResult {
                        step_name: step.name.clone(),
                        output: response.content.clone(),
                        success: true,
                        error: None,
                    });
                    outputs.insert(step.name, response.content);
                }
                Err(err) => {
                    step_results.push(StepResult {
                        step_name: step.name.clone(),
                        output: String::new(),
                        success: false,
                        error: Some(err.to_string()),
                    });
                    break;
                }
            }
        }

        let final_output = outputs.values().last().cloned().unwrap_or_default();

        Ok(WorkflowResult {
            steps: step_results,
            final_output,
        })
    }

    pub fn list_agents(&self) -> Vec<&AgentConfig> {
        self.agents.values().collect()
    }

    pub fn list_knowledge_bases(&self) -> Vec<&String> {
        self.indexes.keys().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_orchestrator_init() {
        let config = AIConfig::default();
        let orchestrator = AIOrchestrator::new(config);
        assert!(orchestrator.list_agents().is_empty());
        assert!(orchestrator.list_knowledge_bases().is_empty());
    }

    #[tokio::test]
    async fn test_register_agent() {
        let config = AIConfig::default();
        let mut orchestrator = AIOrchestrator::new(config);

        let agent = AgentConfig {
            name: "test-agent".to_string(),
            system_prompt: "You are a helpful assistant".to_string(),
            tools: vec![],
            framework: FrameworkType::LangChain,
        };

        assert!(orchestrator.register_agent(agent).await.is_ok());
        assert_eq!(orchestrator.list_agents().len(), 1);
    }

    #[tokio::test]
    async fn test_chat_langchain_placeholder() {
        let config = AIConfig::default();
        let orchestrator = AIOrchestrator::new(config);
        let response = orchestrator
            .chat(vec![], FrameworkType::LangChain)
            .await
            .unwrap();
        assert!(response.content.contains("开发中"));
    }

    #[tokio::test]
    async fn test_chat_llamaindex_unimplemented() {
        let config = AIConfig::default();
        let orchestrator = AIOrchestrator::new(config);
        let result = orchestrator.chat(vec![], FrameworkType::LlamaIndex).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_chat_dspy_unimplemented() {
        let config = AIConfig::default();
        let orchestrator = AIOrchestrator::new(config);
        let result = orchestrator.chat(vec![], FrameworkType::DSPy).await;
        assert!(result.is_err());
    }
}
