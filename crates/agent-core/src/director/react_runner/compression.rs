use super::super::types::{DirectorRuntime, DirectorTraceEntry};
use super::guard::{COMPRESSION_KEEP_RECENT, COMPRESSION_THRESHOLD};
use crate::types::now_millis;

impl DirectorRuntime {
    /// Automatically compress working memory when conversation turns exceed
    /// the configured threshold. Converts old turns into a summary context hint
    /// to prevent context window bloat during long conversations (50+ turns).
    pub fn auto_compress_memory(&mut self) {
        let default_id = crate::memory::AgentMemoryId(0);
        let mem = self.memory_registry.get_mut(default_id);

        if !mem.system.compress_needed(COMPRESSION_THRESHOLD) {
            return;
        }

        let turns_before = mem.system.conversation_turn_count();
        log::info!(
            "Memory compression triggered: {} conversation turns (threshold: {})",
            turns_before,
            COMPRESSION_THRESHOLD
        );

        let compressed = mem.system.compress_working_memory(COMPRESSION_KEEP_RECENT);
        let turns_after = mem.system.conversation_turn_count();

        self.trace_entries.push(DirectorTraceEntry {
            timestamp_ms: now_millis(),
            actor: "MemoryCompressor".into(),
            summary: format!(
                "Compressed {} conversation turns → {} (kept {} recent). {} turns remain.",
                compressed,
                compressed.saturating_sub(turns_before.saturating_sub(turns_after)),
                COMPRESSION_KEEP_RECENT,
                turns_after,
            ),
        });

        log::info!(
            "Memory compressed: {} turns → {} turns ({} compressed)",
            turns_before,
            turns_after,
            compressed,
        );
    }
}
