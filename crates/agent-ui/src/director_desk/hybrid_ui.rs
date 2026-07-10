//! DirectorDesk hybrid UI — HybridController status and banner rendering

use super::types::*;
use bevy_egui::egui;

/// 渲染 HybridEditorController 状态栏
pub(super) fn render_hybrid_status_bar(ui: &mut egui::Ui, state: &mut DirectorDeskState) {
    let mode = &state.hybrid_mode;

    // 状态图标和颜色
    let (icon, status_color) = match mode.status.as_str() {
        "Available" => ("\u{1F7E2}", egui::Color32::from_rgb(16, 185, 129)), // 🟢 绿色
        "Connecting" => ("\u{1F7E1}", egui::Color32::from_rgb(234, 179, 8)), // 🟡 黄色
        "Unavailable" => ("\u{1F534}", egui::Color32::from_rgb(239, 68, 68)), // 🔴 红色
        "Disabled" => ("\u{26AB}", egui::Color32::from_gray(100)),           // ⚫ 黑色
        _ => ("\u{2753}", egui::Color32::GRAY),                              // ❓ 未知
    };

    // 模式标签
    let mode_label = if mode.mode == "LLM" {
        "LLM模式"
    } else {
        "规则引擎"
    };
    let mode_color = if mode.mode == "LLM" {
        egui::Color32::from_rgb(59, 130, 246) // 蓝色
    } else {
        egui::Color32::from_rgb(139, 92, 246) // 紫色
    };

    ui.horizontal(|ui| {
        // 状态图标
        ui.label(egui::RichText::new(icon).size(16.0));

        // 模式名称
        ui.label(
            egui::RichText::new(mode_label)
                .strong()
                .color(mode_color)
                .size(12.0),
        );

        ui.separator();

        // 成功率（仅 LLM 模式显示）
        if mode.mode == "LLM" && mode.success_rate > 0.0 {
            ui.label(
                egui::RichText::new(format!("{:.0}%", mode.success_rate))
                    .size(11.0)
                    .color(status_color),
            );

            // 平均响应时间
            if mode.avg_response_ms > 0.0 {
                ui.label(
                    egui::RichText::new(format!("{:.0}ms", mode.avg_response_ms))
                        .size(10.0)
                        .color(egui::Color32::GRAY),
                );
            }
        } else if mode.mode != "LLM" && mode.consecutive_failures > 0 {
            // 显示降级次数
            ui.label(
                egui::RichText::new(format!("降级x{}", mode.consecutive_failures))
                    .size(11.0)
                    .color(egui::Color32::from_rgb(239, 68, 68)),
            );
        }

        // 下次检测倒计时
        if mode.next_check_countdown > 0.0 {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(format!("{:.0}s", mode.next_check_countdown))
                        .size(10.0)
                        .color(egui::Color32::GRAY),
                );
            });
        }
    });

    // 手动重新检测按钮
    ui.horizontal(|ui| {
        ui.add_space(4.0);

        if ui.button("\u{1F50D} 重新检测").clicked() {
            // 发送重新检测事件（通过 pending_actions）
            state.pending_actions.push(UserAction::RecheckLlm);
        }

        // 如果有降级原因，显示详情按钮
        if mode.fallback_reason.is_some() && ui.button("查看原因").clicked() {
            // 显示详细原因作为横幅
            if let Some(reason) = &mode.fallback_reason {
                state.banner_message = Some(BannerMessage {
                    banner_type: BannerType::Warning,
                    message: format!("降级原因: {}", reason),
                    timestamp: ui.input(|i| i.time),
                    duration: 8.0,
                });
            }
        }
    });

    ui.add_space(4.0);
}

/// 渲染横幅消息（降级/恢复提示）
pub(super) fn render_banner_message(ui: &mut egui::Ui, state: &mut DirectorDeskState) {
    let banner = match &state.banner_message {
        Some(b) => b.clone(),
        None => return,
    };

    let current_time = ui.input(|i| i.time);
    let elapsed = current_time - banner.timestamp;

    if elapsed > banner.duration {
        state.banner_message = None;
        return;
    }

    let (bg_color, text_color, icon) = match banner.banner_type {
        BannerType::Degraded => (
            egui::Color32::from_rgba_unmultiplied(127, 29, 29, 255),
            egui::Color32::WHITE,
            "\u{26A0}",
        ),
        BannerType::Recovered => (
            egui::Color32::from_rgba_unmultiplied(22, 101, 52, 255),
            egui::Color32::WHITE,
            "\u{2705}",
        ),
        BannerType::Warning => (
            egui::Color32::from_rgba_unmultiplied(161, 98, 7, 255),
            egui::Color32::WHITE,
            "\u{26A0}",
        ),
        BannerType::Info => (
            egui::Color32::from_rgba_unmultiplied(30, 64, 175, 255),
            egui::Color32::WHITE,
            "\u{2139}",
        ),
    };

    let alpha = if elapsed > banner.duration - 2.0 {
        ((banner.duration - elapsed) / 2.0) as f32
    } else {
        1.0
    };

    let message = banner.message.clone();
    let mut should_close = false;

    let frame = egui::Frame::NONE
        .fill(bg_color.linear_multiply(alpha))
        .corner_radius(4.0)
        .inner_margin(egui::Margin::symmetric(8, 6));

    frame.show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(icon).size(14.0).color(text_color));
            ui.label(egui::RichText::new(&message).size(11.0).color(text_color));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("\u{2715}").clicked() {
                    should_close = true;
                }
            });
        });
    });

    if should_close {
        state.banner_message = None;
    }

    ui.add_space(4.0);
}

/// 渲染命令面板
pub(super) fn render_command_palette(ctx: &egui::Context, state: &mut DirectorDeskState) {
    if !state.command_palette_open {
        return;
    }

    let mut should_close = false;

    egui::Window::new("Command Palette")
        .id(egui::Id::new("command_palette"))
        .collapsible(false)
        .resizable(false)
        .default_width(400.0)
        .anchor(egui::Align2::CENTER_TOP, [0.0, 80.0])
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("\u{1F50D}").size(16.0));
                let mut search = String::new();
                ui.add_sized(
                    [ui.available_width(), 28.0],
                    egui::TextEdit::singleline(&mut search).hint_text("Type a command..."),
                );
            });

            ui.separator();

            let commands = [
                ("Toggle LLM Mode", "Switch between LLM and Rule engine"),
                ("Recheck LLM", "Manually recheck LLM connection"),
                ("Undo", "Undo last operation"),
                ("Redo", "Redo last undone operation"),
                ("Delete Selected", "Delete selected entity"),
                ("Focus Selected", "Focus camera on selected entity"),
            ];

            for (name, desc) in &commands {
                let response = ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(*name).strong().size(12.0));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new(*desc)
                                .size(10.0)
                                .color(egui::Color32::GRAY),
                        );
                    });
                });

                if response.response.interact(egui::Sense::click()).clicked() {
                    match *name {
                        "Recheck LLM" => state.pending_actions.push(UserAction::RecheckLlm),
                        "Undo" => state.pending_actions.push(UserAction::Undo),
                        "Redo" => state.pending_actions.push(UserAction::Redo),
                        "Delete Selected" => state.pending_actions.push(UserAction::DeleteSelected),
                        "Focus Selected" => state.pending_actions.push(UserAction::FocusSelected),
                        _ => {}
                    }
                    should_close = true;
                }
            }

            ui.separator();
            if ui.button("Close (Esc)").clicked() {
                should_close = true;
            }
        });

    // Close on Escape key
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        should_close = true;
    }

    if should_close {
        state.command_palette_open = false;
    }
}
