// ===========================================================================
// Helper functions for pattern matching
// ===========================================================================

/// Extract entity name from observation text.
pub(crate) fn extract_entity_name_from_obs(obs: &str) -> Option<String> {
    // Try common patterns like "Entity 'Player' already exists"
    if let Some(start) = obs.find('\'') {
        if let Some(end) = obs[start + 1..].find('\'') {
            return Some(obs[start + 1..start + 1 + end].trim().to_string());
        }
    }

    // Try quoted strings
    if let Some(start) = obs.find('"') {
        if let Some(end) = obs[start + 1..].find('"') {
            return Some(obs[start + 1..start + 1 + end].trim().to_string());
        }
    }

    None
}

/// Extract component name from observation text.
pub(crate) fn extract_component_name_from_obs(obs: &str) -> Option<String> {
    let lower = obs.to_lowercase();

    if lower.contains("component") || lower.contains("组件") {
        // Look for word after "component"
        let patterns = [
            "component '",
            "component \"",
            "component ",
            "组件 '",
            "组件 \"",
        ];
        for pattern in &patterns {
            if let Some(start) = lower.find(pattern) {
                let after = &obs[start + pattern.len()..];
                let name: String = after
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if !name.is_empty() {
                    return Some(name);
                }
            }
        }
    }

    None
}

/// Truncate string to max length with ellipsis.
pub(crate) fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}
