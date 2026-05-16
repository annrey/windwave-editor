use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use crate::types::current_timestamp;

/// 观察到的模式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservedPattern {
    pub name: String,
    pub trigger_keywords: Vec<String>,
    pub template: String,
    pub context: String,
    pub observation_count: usize,
    pub last_observed: u64,
    pub success_rate: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidatePattern {
    pub trigger_keywords: Vec<String>,
    pub template: String,
    pub context: String,
    pub observation_count: usize,
    pub successes: usize,
    pub failures: usize,
}

/// 模式学习者 - 从用户操作中自动学习代码模式和项目约定
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternLearner {
    pub patterns: Vec<ObservedPattern>,
    pub min_observations: usize,
    pub candidates: HashMap<String, CandidatePattern>,
}

impl PatternLearner {
    pub fn new(min_observations: usize) -> Self {
        Self {
            patterns: Vec::new(),
            min_observations,
            candidates: HashMap::new(),
        }
    }

    /// 观察一次操作，学习模式
    pub fn observe(&mut self, operation: &str, context: &str, success: bool) {
        let keywords: Vec<String> = operation
            .split_whitespace()
            .map(|s| s.to_lowercase())
            .collect();

        let mut matched = false;
        for pattern in &mut self.patterns {
            if keywords.iter().any(|k| pattern.trigger_keywords.iter().any(|pk| pk == k)) {
                pattern.observation_count += 1;
                pattern.last_observed = current_timestamp();
                if success {
                    let count = pattern.observation_count as f32;
                    pattern.success_rate = (pattern.success_rate * (count - 1.0) + 1.0) / count;
                }
                matched = true;
                break;
            }
        }

        if !matched {
            let key = keywords.join("_");
            let candidate = self.candidates.entry(key.clone()).or_insert(CandidatePattern {
                trigger_keywords: keywords.clone(),
                template: operation.to_string(),
                context: context.to_string(),
                observation_count: 0,
                successes: 0,
                failures: 0,
            });
            candidate.observation_count += 1;
            if success {
                candidate.successes += 1;
            } else {
                candidate.failures += 1;
            }
            if candidate.observation_count >= self.min_observations {
                let pattern = ObservedPattern {
                    name: key.clone(),
                    trigger_keywords: candidate.trigger_keywords.clone(),
                    template: candidate.template.clone(),
                    context: candidate.context.clone(),
                    observation_count: candidate.observation_count,
                    last_observed: current_timestamp(),
                    success_rate: if candidate.observation_count > 0 {
                        candidate.successes as f32 / candidate.observation_count as f32
                    } else {
                        0.0
                    },
                };
                self.patterns.push(pattern);
                self.candidates.remove(&key);
            }
        }
    }

    pub fn suggest(&self, input: &str) -> Vec<&ObservedPattern> {
        let input_lower = input.to_lowercase();
        let mut matches: Vec<&ObservedPattern> = self
            .patterns
            .iter()
            .filter(|p| p.trigger_keywords.iter().any(|k| input_lower.contains(k)))
            .collect();
        matches.sort_by(|a, b| {
            b.success_rate.partial_cmp(&a.success_rate)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| b.observation_count.cmp(&a.observation_count))
        });
        matches
    }

    pub fn describe_patterns(&self, input: &str) -> String {
        let suggestions = self.suggest(input);
        if suggestions.is_empty() {
            return "(no learned patterns match)".into();
        }
        suggestions
            .iter()
            .take(3)
            .map(|p| format!("- {}: {} (成功率: {:.0}%, 观察 {} 次)", p.name, p.template, p.success_rate * 100.0, p.observation_count))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_learner() {
        let mut learner = PatternLearner::new(2);
        learner.observe("create_entity Player", "场景编辑", true);
        learner.observe("create_entity Player", "场景编辑", true);
        let suggestions = learner.suggest("创建一个 Player");
        assert!(!suggestions.is_empty());
    }
}
