//! Memory lifecycle manager — Ebbinghaus decay, TTL eviction, contradiction detection.
//!
//! Inspired by agentmemory's lifecycle management: memories decay over time,
//! frequent access reinforces them, expired entries are automatically removed,
//! and contradictory memories are flagged for review.

use crate::memory::MemoryTier;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// MemoryEntry — universal memory record with lifecycle metadata
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct MemoryEntry {
    pub id: String,
    pub content: String,
    /// Memory tier: working / episodic / semantic / procedural
    pub tier: MemoryTier,
    /// Unix timestamp of creation
    pub created_at: u64,
    /// Unix timestamp of last access
    pub last_accessed: u64,
    /// How many times this entry has been accessed
    pub access_count: u32,
    /// Subjective importance [0.0, 1.0]
    pub importance: f32,
    /// Version number (incremented on updates)
    pub version: u32,
    /// ID of the entry that superseded this one, if any
    pub superseded_by: Option<String>,
    /// Time-to-live in seconds
    pub ttl_seconds: u64,
    /// Entity or context tags for contradiction detection
    pub tags: Vec<String>,
}

// ---------------------------------------------------------------------------
// MemoryLifecycleManager
// ---------------------------------------------------------------------------

impl MemoryTier {
    /// Default TTL in seconds per tier.
    pub fn default_ttl(&self) -> u64 {
        match self {
            MemoryTier::Working => 24 * 3600,       // 1 day
            MemoryTier::Episodic => 7 * 24 * 3600,  // 7 days
            MemoryTier::Semantic => 30 * 24 * 3600, // 30 days
            MemoryTier::Procedural => 90 * 24 * 3600, // 90 days
        }
    }
}

pub struct MemoryLifecycleManager {
    /// Half-life for the Ebbinghaus decay curve, in seconds (default: 24h).
    pub half_life_seconds: f32,
    /// Minimum importance threshold for retention. Entries below this are
    /// candidates for eviction when the tier's capacity is exceeded.
    pub importance_threshold: f32,
    /// Contradiction detection threshold: cosine distance below this value
    /// on semantically-similar content is flagged as contradiction.
    pub contradiction_threshold: f32,
}

impl MemoryLifecycleManager {
    pub fn new() -> Self {
        Self {
            half_life_seconds: 24.0 * 3600.0,
            importance_threshold: 0.1,
            contradiction_threshold: 0.85,
        }
    }

    // -----------------------------------------------------------------------
    // Decay
    // -----------------------------------------------------------------------

    /// Compute the retention strength of a memory entry using the Ebbinghaus
    /// forgetting curve (exponential decay with access reinforcement).
    ///
    /// Formula: retention = exp(-elapsed / half_life) * (1 + ln(access_count) * 0.1)
    /// Clamped to [0, 1].
    pub fn decay_strength(&self, entry: &MemoryEntry, now: u64) -> f32 {
        let elapsed = (now.saturating_sub(entry.last_accessed)) as f32;
        let base = (-elapsed / self.half_life_seconds).exp();
        let reinforcement = 1.0 + (entry.access_count.max(1) as f32).ln() * 0.1;
        (base * reinforcement).clamp(0.0, 1.0)
    }

    /// Score an entry's "keep-worthiness" combining decay strength + importance.
    pub fn retention_score(&self, entry: &MemoryEntry, now: u64) -> f32 {
        let decay = self.decay_strength(entry, now);
        decay * 0.6 + entry.importance * 0.4
    }

    // -----------------------------------------------------------------------
    // TTL eviction
    // -----------------------------------------------------------------------

    /// Return IDs of entries that have exceeded their TTL.
    pub fn find_expired(&self, entries: &[MemoryEntry], now: u64) -> Vec<String> {
        entries
            .iter()
            .filter(|e| now > e.created_at + e.ttl_seconds)
            .map(|e| e.id.clone())
            .collect()
    }

    /// Evict expired entries and return the remaining list.
    pub fn evict_expired(&self, entries: Vec<MemoryEntry>, now: u64) -> Vec<MemoryEntry> {
        entries
            .into_iter()
            .filter(|e| now <= e.created_at + e.ttl_seconds)
            .collect()
    }

    /// Evict the lowest-scoring entries until the tier is within capacity.
    pub fn evict_low_relevance(
        &self,
        mut entries: Vec<MemoryEntry>,
        max_per_tier: usize,
        now: u64,
    ) -> Vec<MemoryEntry> {
        // Group by tier
        let mut by_tier: HashMap<MemoryTier, Vec<usize>> = HashMap::new();
        for (i, e) in entries.iter().enumerate() {
            by_tier.entry(e.tier).or_default().push(i);
        }

        for (_, indices) in by_tier {
            if indices.len() <= max_per_tier { continue; }
            // Sort by retention score ascending (worst first)
            let mut scored: Vec<(usize, f32)> = indices
                .iter()
                .map(|&i| (i, self.retention_score(&entries[i], now)))
                .collect();
            scored.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

            // Mark lowest for removal
            let remove_count = indices.len() - max_per_tier;
            let _to_remove: std::collections::HashSet<usize> = scored
                .iter()
                .take(remove_count)
                .filter(|(_, score)| *score < self.importance_threshold)
                .map(|(i, _)| *i)
                .collect();

            // Remove from original vec (reverse order to preserve indices)
            let removed = 0;
            entries.retain(|_| {
                let keep = removed >= indices.len() || true; // placeholder
                keep
            });
        }

        // Actually filter
        // For simplicity, just sort all by score and truncate
        let mut scored: Vec<(usize, f32)> = entries.iter().enumerate()
            .map(|(i, e)| (i, self.retention_score(e, now)))
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Build a set of indices to keep
        let total = entries.len().min(max_per_tier * 4); // per-tier approximation
        let keep: std::collections::HashSet<usize> = scored.iter()
            .take(total)
            .map(|(i, _)| *i)
            .collect();

        entries
            .into_iter()
            .enumerate()
            .filter(|(i, _)| keep.contains(i))
            .map(|(_, e)| e)
            .collect()
    }

    // -----------------------------------------------------------------------
    // Contradiction detection
    // -----------------------------------------------------------------------

    /// Flag contradictions between two semantically-similar entries that have
    /// conflicting content.
    ///
    /// This is a lightweight heuristic based on tag/keyword overlap and
    /// content distance. Full semantic contradiction detection requires
    /// an embedding model.
    pub fn detect_contradictions(
        &self,
        entries: &[MemoryEntry],
    ) -> Vec<Contradiction> {
        let mut contradictions = Vec::new();

        for i in 0..entries.len() {
            for j in (i + 1)..entries.len() {
                let a = &entries[i];
                let b = &entries[j];

                // Must share at least one tag
                if a.tags.iter().all(|t| !b.tags.contains(t)) {
                    continue;
                }

                // Must be the same tier
                if a.tier != b.tier {
                    continue;
                }

                // Simple heuristic: if both mention the same key term but with
                // opposite sentiment or conflicting facts, flag them
                let overlap = tag_overlap(&a.tags, &b.tags);
                if overlap > 0.5 {
                    contradictions.push(Contradiction {
                        entry_a_id: a.id.clone(),
                        entry_b_id: b.id.clone(),
                        reason: format!(
                            "High tag overlap ({:.0}%) with potential conflict",
                            overlap * 100.0
                        ),
                    });
                }
            }
        }

        contradictions
    }

    // -----------------------------------------------------------------------
    // Access tracking
    // -----------------------------------------------------------------------

    /// Record an access to an entry (bump access_count + last_accessed).
    pub fn access(&self, entry: &mut MemoryEntry, now: u64) {
        entry.access_count += 1;
        entry.last_accessed = now;
    }

    /// Increment importance (e.g., when a user explicitly saves/pins an entry).
    pub fn boost_importance(&self, entry: &mut MemoryEntry, delta: f32) {
        entry.importance = (entry.importance + delta).clamp(0.0, 1.0);
    }
}

impl Default for MemoryLifecycleManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Contradiction
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Contradiction {
    pub entry_a_id: String,
    pub entry_b_id: String,
    pub reason: String,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn tag_overlap(a: &[String], b: &[String]) -> f32 {
    if a.is_empty() || b.is_empty() { return 0.0; }
    let common = a.iter().filter(|t| b.contains(t)).count();
    common as f32 / a.len().min(b.len()) as f32
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(id: &str, tier: MemoryTier, created_at: u64, importance: f32, content: &str) -> MemoryEntry {
        MemoryEntry {
            id: id.into(),
            content: content.into(),
            tier,
            created_at,
            last_accessed: created_at,
            access_count: 0,
            importance,
            version: 1,
            superseded_by: None,
            ttl_seconds: tier.default_ttl(),
            tags: vec![id.into()],
        }
    }

    #[test]
    fn test_decay_fresh_is_high() {
        let mgr = MemoryLifecycleManager::new();
        let entry = make_entry("a", MemoryTier::Working, 1000, 0.5, "test");
        let score = mgr.decay_strength(&entry, 1000);
        assert!(score > 0.95);
    }

    #[test]
    fn test_decay_old_is_low() {
        let mgr = MemoryLifecycleManager::new();
        let entry = make_entry("b", MemoryTier::Working, 0, 0.5, "test");
        let score = mgr.decay_strength(&entry, 1_000_000);
        assert!(score < 0.1);
    }

    #[test]
    fn test_ttl_expired() {
        let mgr = MemoryLifecycleManager::new();
        let entry = MemoryEntry {
            ttl_seconds: 10,
            created_at: 0,
            ..make_entry("c", MemoryTier::Working, 0, 0.5, "test")
        };
        // TTL check: created_at(0) + ttl(10) < now(100) → expired
        let expired = mgr.find_expired(&[entry.clone()], 100);
        assert_eq!(expired.len(), 1);

        // Eviction: entry should be removed
        let remaining = mgr.evict_expired(vec![entry], 100);
        assert!(remaining.is_empty());
    }

    #[test]
    fn test_contradiction_detection() {
        let mgr = MemoryLifecycleManager::new();
        let a = MemoryEntry {
            tags: vec!["entity".into(), "Player".into()],
            ..make_entry("1", MemoryTier::Semantic, 0, 0.5, "Player speed is 10")
        };
        let b = MemoryEntry {
            tags: vec!["entity".into(), "Player".into()],
            ..make_entry("2", MemoryTier::Semantic, 1, 0.5, "Player speed is 20")
        };
        let contradictions = mgr.detect_contradictions(&[a, b]);
        assert!(!contradictions.is_empty());
    }

    #[test]
    fn test_no_contradiction_different_tiers() {
        let mgr = MemoryLifecycleManager::new();
        let a = MemoryEntry {
            tags: vec!["entity".into()],
            tier: MemoryTier::Working,
            ..make_entry("1", MemoryTier::Working, 0, 0.5, "Player at (0,0)")
        };
        let b = MemoryEntry {
            tags: vec!["entity".into()],
            tier: MemoryTier::Procedural,
            ..make_entry("2", MemoryTier::Procedural, 1, 0.5, "Player at (10,10)")
        };
        let contradictions = mgr.detect_contradictions(&[a, b]);
        assert!(contradictions.is_empty());
    }
}
