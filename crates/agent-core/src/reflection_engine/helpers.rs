//! Helper Functions

/// Extract entity name from error message.
pub(crate) fn extract_entity_from_error(error: &str) -> String {
    // Try single quotes: Entity 'Player'
    if let Some(start) = error.find('\'') {
        if let Some(end) = error[start + 1..].find('\'') {
            return error[start + 1..start + 1 + end].trim().to_string();
        }
    }

    // Try double quotes: Entity "Player"
    if let Some(start) = error.find('"') {
        if let Some(end) = error[start + 1..].find('"') {
            return error[start + 1..start + 1 + end].trim().to_string();
        }
    }

    "unknown".to_string()
}

/// Extract parameter name from error message.
pub(crate) fn extract_parameter_name(error: &str) -> Option<String> {
    let patterns = [
        "parameter '",
        "parameter \"",
        "argument '",
        "argument \"",
        "field '",
        "field \"",
        "value '",
        "value \"",
    ];

    for pattern in &patterns {
        if let Some(start) = error.find(pattern) {
            let after = &error[start + pattern.len()..];
            let name: String = after
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            if !name.is_empty() {
                return Some(name);
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
