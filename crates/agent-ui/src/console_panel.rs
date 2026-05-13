//! Console Panel - Agent execution logs
//!
//! Provides a bottom panel for viewing Agent execution logs and system messages.
//! Features:
//! - Filter by log level (Info, Warning, Error)
//! - Filter by source (Director, Agent, System)
//! - Auto-scroll to latest messages
//! - Search within logs

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use crate::layout::LayoutManager;
use std::collections::VecDeque;

pub struct ConsolePanelPlugin;

impl Plugin for ConsolePanelPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ConsoleState>()
            .add_systems(Update, render_console_panel);
    }
}

#[derive(Resource)]
pub struct ConsoleState {
    pub entries: VecDeque<ConsoleEntry>,
    pub filter: LogFilter,
    pub auto_scroll: bool,
    pub search_query: String,
    pub max_entries: usize,
}

impl Default for ConsoleState {
    fn default() -> Self {
        Self {
            entries: VecDeque::with_capacity(1000),
            filter: LogFilter::All,
            auto_scroll: true,
            search_query: String::new(),
            max_entries: 1000,
        }
    }
}

pub struct ConsoleEntry {
    pub timestamp: String,
    pub level: LogLevel,
    pub source: String,
    pub message: String,
}

#[derive(Clone, Copy, PartialEq, Default)]
pub enum LogFilter {
    #[default]
    All,
    Info,
    Warning,
    Error,
    Agent,
    System,
}

#[derive(Clone, Copy, PartialEq)]
pub enum LogLevel {
    Debug,
    Info,
    Warning,
    Error,
}

impl LogLevel {
    fn color(&self) -> egui::Color32 {
        match self {
            LogLevel::Debug => egui::Color32::GRAY,
            LogLevel::Info => egui::Color32::WHITE,
            LogLevel::Warning => egui::Color32::YELLOW,
            LogLevel::Error => egui::Color32::RED,
        }
    }
}

fn render_console_panel(
    mut contexts: EguiContexts,
    mut state: ResMut<ConsoleState>,
    layout_mgr: Res<LayoutManager>,
) {
    if !layout_mgr.is_visible("console") { return; }

    let ctx = contexts.ctx_mut();
    let Ok(ctx) = ctx else { return };

    egui::TopBottomPanel::bottom("console_panel")
        .default_height(200.0)
        .resizable(true)
        .show(ctx, |ui| {
            // Toolbar
            ui.horizontal(|ui| {
                ui.heading("Console");
                ui.separator();

                // Filter buttons
                ui.selectable_value(&mut state.filter, LogFilter::All, "All");
                ui.selectable_value(&mut state.filter, LogFilter::Info, "Info");
                ui.selectable_value(&mut state.filter, LogFilter::Warning, "Warnings");
                ui.selectable_value(&mut state.filter, LogFilter::Error, "Errors");
                ui.selectable_value(&mut state.filter, LogFilter::Agent, "Agent");
                ui.selectable_value(&mut state.filter, LogFilter::System, "System");

                ui.separator();

                // Search
                ui.label("🔍");
                ui.text_edit_singleline(&mut state.search_query);

                ui.separator();

                // Auto-scroll toggle
                ui.checkbox(&mut state.auto_scroll, "Auto-scroll");

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Clear").clicked() {
                        state.entries.clear();
                    }
                });
            });

            ui.separator();

            // Log entries
            let text_style = egui::TextStyle::Monospace;
            let row_height = ui.text_style_height(&text_style);

            let filtered_entries: Vec<&ConsoleEntry> = state
                .entries
                .iter()
                .filter(|entry| {
                    // Apply level filter
                    let level_match = match state.filter {
                        LogFilter::All => true,
                        LogFilter::Info => matches!(entry.level, LogLevel::Info),
                        LogFilter::Warning => matches!(entry.level, LogLevel::Warning),
                        LogFilter::Error => matches!(entry.level, LogLevel::Error),
                        LogFilter::Agent => entry.source.contains("Agent"),
                        LogFilter::System => entry.source.contains("System"),
                    };

                    // Apply search filter
                    let search_match = if state.search_query.is_empty() {
                        true
                    } else {
                        entry.message.to_lowercase().contains(&state.search_query.to_lowercase())
                    };

                    level_match && search_match
                })
                .collect();

            let total_rows = filtered_entries.len();

            egui::ScrollArea::vertical()
                .stick_to_bottom(state.auto_scroll)
                .show_rows(ui, row_height, total_rows, |ui, row_range| {
                    for i in row_range {
                        if let Some(entry) = filtered_entries.get(i) {
                            render_log_entry(ui, entry);
                        }
                    }
                });
        });
}

fn render_log_entry(ui: &mut egui::Ui, entry: &ConsoleEntry) {
    ui.horizontal(|ui| {
        // Timestamp
        ui.label(
            egui::RichText::new(&entry.timestamp)
                .monospace()
                .color(egui::Color32::GRAY)
        );

        // Source
        ui.label(
            egui::RichText::new(format!("[{}]", entry.source))
                .monospace()
                .color(egui::Color32::LIGHT_BLUE)
        );

        // Level indicator
        let level_icon = match entry.level {
            LogLevel::Debug => "🔍",
            LogLevel::Info => "ℹ️",
            LogLevel::Warning => "⚠️",
            LogLevel::Error => "❌",
        };
        ui.label(level_icon);

        // Message
        ui.label(
            egui::RichText::new(&entry.message)
                .monospace()
                .color(entry.level.color())
        );
    });
}

/// Helper function to add a log entry from other systems
pub fn log_console(
    console: &mut ConsoleState,
    level: LogLevel,
    source: &str,
    message: &str,
) {
    let timestamp = format!("{:.3}", bevy::time::Time::new_with(0.0).elapsed_secs());

    console.entries.push_back(ConsoleEntry {
        timestamp,
        level,
        source: source.to_string(),
        message: message.to_string(),
    });

    // Trim if exceeds max
    while console.entries.len() > console.max_entries {
        console.entries.pop_front();
    }
}
