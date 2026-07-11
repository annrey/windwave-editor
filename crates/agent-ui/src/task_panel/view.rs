//! Task panel egui rendering.

use bevy::prelude::{Res, ResMut};
use bevy_egui::{egui, EguiContexts};

use crate::layout::{LayoutManager, PanelPosition};

use super::model::{SortBy, SyncStatus, TaskFilter, TaskPanelState, TaskStatus};
use super::port::TaskPanelCommand;

pub(super) fn render_task_panel(
    mut contexts: EguiContexts,
    mut task_state: ResMut<TaskPanelState>,
    layout_mgr: Res<LayoutManager>,
) {
    if !layout_mgr.is_visible("task") {
        return;
    }

    let ctx = match contexts.ctx_mut() {
        Ok(c) => c,
        Err(_) => return,
    };

    // ── P2-6: Keyboard shortcuts ──
    let mut kb_ctrl_n = false;
    let mut kb_delete = false;
    let mut kb_escape = false;
    ctx.input(|i| {
        if i.modifiers.ctrl && i.key_pressed(egui::Key::N) {
            kb_ctrl_n = true;
        }
        if i.key_pressed(egui::Key::Delete) {
            kb_delete = true;
        }
        if i.key_pressed(egui::Key::Escape) {
            kb_escape = true;
        }
    });

    // Process keyboard shortcuts
    if kb_ctrl_n {
        task_state.open_create_dialog();
    }
    if kb_escape {
        task_state.close_create_dialog();
        task_state.selected_task = None;
        task_state.show_delete_confirm = None;
        task_state.selected_ids.clear();
        task_state.select_mode = false;
    }
    if kb_delete {
        if let Some(ref selected_id) = task_state.selected_task.clone() {
            task_state.show_delete_confirm = Some(selected_id.clone());
        }
    }

    let (win_w, win_h) = layout_mgr
        .panel_config("task")
        .and_then(|c| match c.position {
            PanelPosition::Floating { width, height, .. } => Some((width, height)),
            _ => None,
        })
        .unwrap_or((450.0, 650.0));

    egui::Window::new("任务面板")
        .default_size([win_w, win_h])
        .resizable(true)
        .show(ctx, |ui| {
            // ── 顶部工具栏 ──
            ui.horizontal(|ui| {
                // 搜索栏
                ui.horizontal(|ui| {
                    ui.label("\u{1F50D}"); // P2-8: unicode escape for 🔍
                    egui::TextEdit::singleline(&mut task_state.search_query)
                        .hint_text("搜索任务标题或描述...")
                        .show(ui);
                });

                // 刷新按钮 (P2-7: tooltip)
                if ui
                    .button("\u{1F504}") // P2-8: unicode escape for 🔄
                    .on_hover_text("刷新任务列表")
                    .clicked()
                {
                    task_state.pending_commands.push(TaskPanelCommand::Refresh);
                }

                // ── P2-2: Sync status icon ──
                let (sync_icon, sync_tooltip) = match &task_state.sync_status {
                    SyncStatus::NotSynced => ("\u{26AA}", "未同步"),
                    SyncStatus::Syncing => ("\u{1F7E1}", "同步中"),
                    SyncStatus::Synced => ("\u{1F7E2}", "已同步"),
                    SyncStatus::SyncError(msg) => ("\u{1F534}", msg.as_str()),
                };
                ui.label(sync_icon).on_hover_text(sync_tooltip);

                // 创建任务按钮 (P2-7: tooltip)
                if ui
                    .button("+ 新建")
                    .on_hover_text("创建新任务 (Ctrl+N)")
                    .clicked()
                {
                    task_state.open_create_dialog();
                }

                // ── P2-4: Select mode toggle ──
                let select_label = if task_state.select_mode {
                    "退出选择"
                } else {
                    "批量选择"
                };
                if ui.button(select_label).clicked() {
                    task_state.select_mode = !task_state.select_mode;
                    if !task_state.select_mode {
                        task_state.selected_ids.clear();
                    }
                }
            });

            ui.separator();

            // ── 筛选按钮 + 排序下拉 ──
            ui.horizontal_wrapped(|ui| {
                ui.label("筛选:");
                ui.selectable_value(&mut task_state.filter, TaskFilter::All, "全部");
                ui.selectable_value(
                    &mut task_state.filter,
                    TaskFilter::ByStatus(TaskStatus::Pending),
                    "待处理",
                );
                ui.selectable_value(
                    &mut task_state.filter,
                    TaskFilter::ByStatus(TaskStatus::InProgress),
                    "进行中",
                );
                ui.selectable_value(
                    &mut task_state.filter,
                    TaskFilter::ByStatus(TaskStatus::Done),
                    "已完成",
                );
                ui.selectable_value(
                    &mut task_state.filter,
                    TaskFilter::ByPriority(4),
                    "高优先级",
                );

                // ── P2-1: Sort dropdown ──
                ui.add_space(12.0);
                egui::ComboBox::from_label("排序")
                    .selected_text(match task_state.sort_by {
                        SortBy::Priority => "优先级",
                        SortBy::CreatedAt => "创建时间",
                        SortBy::Status => "状态",
                        SortBy::Title => "标题",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut task_state.sort_by, SortBy::Priority, "优先级");
                        ui.selectable_value(&mut task_state.sort_by, SortBy::CreatedAt, "创建时间");
                        ui.selectable_value(&mut task_state.sort_by, SortBy::Status, "状态");
                        ui.selectable_value(&mut task_state.sort_by, SortBy::Title, "标题");
                    })
                    .response
                    .on_hover_text("选择排序方式");
            });

            ui.separator();

            // ── 任务列表 ──
            egui::ScrollArea::vertical().show(ui, |ui| {
                let filtered: Vec<_> = task_state
                    .get_filtered_tasks()
                    .into_iter()
                    .cloned()
                    .collect();

                if filtered.is_empty() {
                    // ── P2-3: Empty state guidance ──
                    if task_state.tasks.is_empty() && task_state.total_count == 0 {
                        ui.centered_and_justified(|ui| {
                            ui.vertical(|ui| {
                                ui.label("暂无任务");
                                ui.add_space(8.0);
                                if ui.button("创建第一个任务").clicked() {
                                    task_state.open_create_dialog();
                                }
                            });
                        });
                    } else {
                        ui.centered_and_justified(|ui| {
                            ui.vertical(|ui| {
                                ui.label("没有匹配的任务");
                                ui.add_space(8.0);
                                if ui.button("清除筛选").clicked() {
                                    task_state.filter = TaskFilter::All;
                                    task_state.search_query.clear();
                                }
                            });
                        });
                    }
                } else {
                    // ── P2-4: Batch operations toolbar ──
                    if task_state.select_mode && !task_state.selected_ids.is_empty() {
                        ui.horizontal(|ui| {
                            ui.label(format!("已选择 {} 项", task_state.selected_ids.len()));
                            if ui.button("批量开始").clicked() {
                                for id in task_state.selected_ids.clone() {
                                    task_state.pending_commands.push(
                                        TaskPanelCommand::UpdateStatus {
                                            id: id.clone(),
                                            status: TaskStatus::InProgress,
                                        },
                                    );
                                    task_state.update_task_status(&id, TaskStatus::InProgress);
                                }
                                task_state.selected_ids.clear();
                            }
                            if ui.button("批量完成").clicked() {
                                for id in task_state.selected_ids.clone() {
                                    task_state.pending_commands.push(
                                        TaskPanelCommand::UpdateStatus {
                                            id: id.clone(),
                                            status: TaskStatus::Done,
                                        },
                                    );
                                    task_state.update_task_status(&id, TaskStatus::Done);
                                }
                                task_state.selected_ids.clear();
                            }
                            if ui.button("批量删除").clicked() {
                                // Show confirmation for the first selected task as a proxy
                                // (batch delete confirms all)
                                for id in task_state.selected_ids.clone() {
                                    task_state
                                        .pending_commands
                                        .push(TaskPanelCommand::Delete { id: id.clone() });
                                }
                                task_state.selected_ids.clear();
                            }
                            if ui.button("取消选择").clicked() {
                                task_state.selected_ids.clear();
                            }
                        });
                        ui.separator();
                    }

                    for task in &filtered {
                        let is_selected = task_state.selected_task.as_deref() == Some(&task.id);

                        // ── P2-5: Card visual hierarchy with ui.group ──
                        ui.group(|ui| {
                            // Row 1: Status indicator + Title
                            ui.horizontal(|ui| {
                                // ── P2-4: Checkbox in select mode ──
                                if task_state.select_mode {
                                    let mut is_checked = task_state.selected_ids.contains(&task.id);
                                    if ui.checkbox(&mut is_checked, "").changed() {
                                        if is_checked {
                                            task_state.selected_ids.push(task.id.clone());
                                        } else {
                                            task_state.selected_ids.retain(|id| id != &task.id);
                                        }
                                    }
                                }

                                let color_utf8 = match task.status {
                                    TaskStatus::Pending => "\u{26AB}",
                                    TaskStatus::InProgress => "\u{1F7E1}",
                                    TaskStatus::Done => "\u{1F7E2}",
                                    TaskStatus::Failed => "\u{1F534}",
                                    TaskStatus::Cancelled => "\u{26AA}",
                                };
                                ui.label(format!("{} {}", color_utf8, task.status.display()));

                                ui.label(egui::RichText::new(&task.title).strong().color(
                                    if is_selected {
                                        egui::Color32::from_rgb(100, 150, 255)
                                    } else {
                                        egui::Color32::WHITE
                                    },
                                ));
                            });

                            // Row 2: Description (small, gray)
                            if !task.description.is_empty() {
                                ui.label(
                                    egui::RichText::new(&task.description)
                                        .size(10.0)
                                        .color(egui::Color32::GRAY),
                                );
                            }

                            // Row 3: Priority badge + Scene badge + Multica badge
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(format!("P{}", task.priority))
                                        .size(9.0)
                                        .color(egui::Color32::YELLOW),
                                );

                                if let Some(scene_id) = &task.scene_id {
                                    ui.label(
                                        egui::RichText::new(format!("[场景: {}]", scene_id))
                                            .size(9.0)
                                            .color(egui::Color32::DARK_GREEN),
                                    );
                                }

                                if task.multica_id.is_some() {
                                    ui.label(
                                        egui::RichText::new("[Multica]")
                                            .size(9.0)
                                            .color(egui::Color32::LIGHT_BLUE),
                                    );
                                }
                            });

                            // Row 4: Action buttons (P2-7: tooltips)
                            ui.horizontal(|ui| {
                                if ui
                                    .small_button("选择")
                                    .on_hover_text("选择此任务查看详情")
                                    .clicked()
                                {
                                    task_state.selected_task = Some(task.id.clone());
                                }

                                ui.add_enabled_ui(task.status == TaskStatus::Pending, |ui| {
                                    if ui
                                        .small_button("开始")
                                        .on_hover_text("开始执行此任务")
                                        .clicked()
                                    {
                                        task_state.pending_commands.push(
                                            TaskPanelCommand::UpdateStatus {
                                                id: task.id.clone(),
                                                status: TaskStatus::InProgress,
                                            },
                                        );
                                        task_state
                                            .update_task_status(&task.id, TaskStatus::InProgress);
                                    }
                                });

                                ui.add_enabled_ui(task.status == TaskStatus::InProgress, |ui| {
                                    if ui
                                        .small_button("完成")
                                        .on_hover_text("标记任务为已完成")
                                        .clicked()
                                    {
                                        task_state.pending_commands.push(
                                            TaskPanelCommand::UpdateStatus {
                                                id: task.id.clone(),
                                                status: TaskStatus::Done,
                                            },
                                        );
                                        task_state.update_task_status(&task.id, TaskStatus::Done);
                                    }
                                });

                                if ui
                                    .small_button("删除")
                                    .on_hover_text("删除此任务 (Del)")
                                    .clicked()
                                {
                                    task_state.show_delete_confirm = Some(task.id.clone());
                                }
                            });
                        });

                        ui.add_space(4.0);
                    }
                }
            });

            // ── P1-2: Selected task detail panel ──
            if let Some(selected_id) = &task_state.selected_task.clone() {
                if let Some(task) = task_state.tasks.get(selected_id) {
                    ui.separator();
                    ui.collapsing("任务详情", |ui| {
                        ui.label(format!("ID: {}", task.id));
                        ui.label(format!("描述: {}", task.description));
                        ui.label(format!("状态: {}", task.status.display()));
                        ui.label(format!("创建时间: {}", task.created_at));
                        ui.label(format!("关联实体: {}", task.entity_ids.len()));
                        if let Some(ref mid) = task.multica_id {
                            ui.label(format!("Multica ID: {}", mid));
                        }
                    });
                }
            }

            ui.separator();

            // ── P1-5: Stats bar layout (always show all 5 statuses) ──
            ui.horizontal(|ui| {
                let total = task_state.total_count;
                ui.label(egui::RichText::new(format!("总计: {}", total)).strong());

                let statuses = [
                    (TaskStatus::Pending, "待处理"),
                    (TaskStatus::InProgress, "进行中"),
                    (TaskStatus::Done, "已完成"),
                    (TaskStatus::Failed, "失败"),
                    (TaskStatus::Cancelled, "已取消"),
                ];
                for (status, label) in &statuses {
                    let count = task_state.status_counts.get(status).copied().unwrap_or(0);
                    let [r, g, b] = status.color();
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new(format!("{}:{}", label, count)).color(
                        egui::Color32::from_rgb(
                            (r * 255.0) as u8,
                            (g * 255.0) as u8,
                            (b * 255.0) as u8,
                        ),
                    ));
                }
            });

            // ── P1-3: Inline create dialog (collapsing) ──
            let collapsing_id = ui.make_persistent_id("task_create_section");
            let mut collapsing_state =
                egui::collapsing_header::CollapsingState::load_with_default_open(
                    ui.ctx(),
                    collapsing_id,
                    task_state.show_create_dialog,
                );
            if task_state.show_create_dialog {
                collapsing_state.set_open(true);
            }

            let header_response = ui.collapsing("创建新任务", |ui| {
                ui.vertical(|ui| {
                    egui::TextEdit::singleline(&mut task_state.new_task_title)
                        .hint_text("标题")
                        .show(ui);

                    egui::TextEdit::multiline(&mut task_state.new_task_description)
                        .hint_text("描述")
                        .desired_rows(3)
                        .desired_width(350.0)
                        .show(ui);

                    egui::TextEdit::singleline(&mut task_state.new_task_scene_id)
                        .hint_text("场景 ID (可选)")
                        .show(ui);

                    // P1-7: Priority labels
                    ui.horizontal(|ui| {
                        ui.label("优先级:");
                        let priorities =
                            [(1, "低"), (2, "较低"), (3, "中"), (4, "高"), (5, "紧急")];
                        for (p, label) in &priorities {
                            ui.selectable_value(&mut task_state.new_task_priority, *p, *label)
                                .on_hover_text(format!("优先级 {}", p));
                        }
                    });

                    ui.horizontal(|ui| {
                        if ui.button("创建").on_hover_text("提交创建").clicked() {
                            task_state.submit_new_task();
                        }
                        if ui.button("取消").on_hover_text("取消创建").clicked() {
                            task_state.close_create_dialog();
                        }
                    });
                });
            });

            // Sync collapsed state: if user closed the collapsing, set show_create_dialog = false
            if task_state.show_create_dialog && !collapsing_state.is_open() {
                task_state.show_create_dialog = false;
            }
            // Silence unused warning
            let _ = header_response;
        });

    // ── P0-2: Delete confirmation dialog ──
    if let Some(ref delete_id) = task_state.show_delete_confirm.clone() {
        let title = task_state
            .tasks
            .get(delete_id)
            .map(|t| t.title.clone())
            .unwrap_or_default();

        egui::Window::new("确认删除")
            .collapsible(false)
            .resizable(false)
            .default_width(300.0)
            .show(ctx, |ui| {
                ui.label(format!("确认删除任务「{}」?", title));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("确认删除").clicked() {
                        task_state.pending_commands.push(TaskPanelCommand::Delete {
                            id: delete_id.clone(),
                        });
                        task_state.show_delete_confirm = None;
                    }
                    if ui.button("取消").clicked() {
                        task_state.show_delete_confirm = None;
                    }
                });
            });
    }
}
