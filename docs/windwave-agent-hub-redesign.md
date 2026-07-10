# WindWave Agent 中枢桥接 + 黑暗玩家风格改造文档

> 版本: v0.3.0 设计草案  
> 日期: 2026-06-03  
> 状态: 设计阶段

---

## 1. 愿景

将 WindWave 从"内嵌 AI 的游戏编辑器"升级为 **Agent 中枢操控台（Agent Command Deck）**——Agent 不再只是编辑器内部的对话助手，而是作为**中枢桥接器**，直接操控 Blender、Godot、VS Code、Figma、Terminal 等各类开源软件。所有操作汇聚到一个**黑暗玩家风格的统一操控界面**中。

---

## 2. 核心架构变更

### 2.1 当前架构

```
┌─────────────────────────────────────────────────────┐
│                 WindWave Editor                      │
│  ┌──────────┐  ┌──────────┐  ┌──────────────────┐  │
│  │ Agent    │  │  Bevy    │  │  egui UI         │  │
│  │ (Core)   │──│  Adapter │──│  (Panels)        │  │
│  └──────────┘  └──────────┘  └──────────────────┘  │
│       │              │                               │
│       └──────┬───────┘                               │
│              │                                       │
│     ┌────────┴────────┐                              │
│     │  Multica Bridge │                              │
│     └────────┬────────┘                              │
└──────────────┼──────────────────────────────────────┘
               │
        ┌──────┴──────┐
        │   Multica   │
        │   Server    │
        └─────────────┘
```

Agent 只能通过内部 Bevy adapter 操作场景，或通过 Multica bridge 与外部的 Multica 协作。

### 2.2 目标架构

```
┌─────────────────────────────────────────────────────────────────┐
│                    WindWave Agent Command Deck                    │
│                    (黑暗玩家风格 UI)                                │
│                                                                   │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │                    Director AI Core                          │ │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────────┐  │ │
│  │  │ Planner  │ │ Memory   │ │ Skills   │ │ Reflection   │  │ │
│  │  └──────────┘ └──────────┘ └──────────┘ └──────────────┘  │ │
│  └────────────────────────┬────────────────────────────────────┘ │
│                           │                                      │
│  ┌────────────────────────┼────────────────────────────────────┐ │
│  │              Agent Hub Bridge (中枢桥接层)                    │ │
│  │  ┌──────────────────────────────────────────────────────┐   │ │
│  │  │           Tool Orchestrator (工具编排器)               │   │ │
│  │  │  ┌────────┐ ┌────────┐ ┌────────┐ ┌──────────────┐  │   │ │
│  │  │  │ 任务分发│ │ 上下文 │ │ 结果   │ │ 冲突检测     │  │   │ │
│  │  │  │ 路由   │ │ 聚合   │ │ 合并   │ │ 与仲裁       │  │   │ │
│  │  │  └────────┘ └────────┘ └────────┘ └──────────────┘  │   │ │
│  │  └──────────────────────────────────────────────────────┘   │ │
│  │                           │                                  │ │
│  │  ┌────────────────────────┼──────────────────────────────┐  │ │
│  │  │              Tool Adapters (工具适配器)                 │  │ │
│  │  │                                                        │  │ │
│  │  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐ │  │ │
│  │  │  │ Blender  │ │  Godot   │ │ VS Code  │ │ Terminal │ │  │ │
│  │  │  │ Adapter  │ │ Adapter  │ │ Adapter  │ │ Adapter  │ │  │ │
│  │  │  └────┬─────┘ └────┬─────┘ └────┬─────┘ └────┬─────┘ │  │ │
│  │  │       │            │            │            │        │  │ │
│  │  │  ┌────┴─────┐ ┌────┴─────┐ ┌────┴─────┐ ┌────┴─────┐ │  │ │
│  │  │  │ Figma   │ │ Unity   │ │  Bevy    │ │  Git     │ │  │ │
│  │  │  │ Adapter │ │ Adapter │ │ Adapter  │ │ Adapter  │ │  │ │
│  │  │  └─────────┘ └─────────┘ └──────────┘ └──────────┘ │  │ │
│  │  └──────────────────────────────────────────────────────┘  │ │
│  └────────────────────────────────────────────────────────────┘ │
│                           │                                      │
│  ┌────────────────────────┼────────────────────────────────────┐ │
│  │              UI Layer (黑暗玩家风格)                          │ │
│  │  ┌─────────┐ ┌─────────┐ ┌──────────┐ ┌────────────────┐   │ │
│  │  │ Command │ │ Tool    │ │ Scene    │ │ Live Preview   │   │ │
│  │  │ Palette │ │ Dock    │ │ Viewport │ │ (Multi-Window) │   │ │
│  │  └─────────┘ └─────────┘ └──────────┘ └────────────────┘   │ │
│  └────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
         │            │            │            │
    ┌────┴────┐  ┌────┴────┐  ┌────┴────┐  ┌────┴────┐
    │ Blender │  │  Godot  │  │ VS Code │  │ Terminal│
    │  (MCP)  │  │  (MCP)  │  │  (MCP)  │  │  (MCP)  │
    └─────────┘  └─────────┘  └─────────┘  └─────────┘
```

---

## 3. Agent Hub Bridge 设计

### 3.1 工具适配器协议（Tool Adapter Protocol）

每个外部工具通过统一的 **Tool Adapter Protocol** 接入：

```rust
/// 工具适配器 trait —— 所有外部工具接入的统一接口
#[async_trait]
pub trait ToolAdapter: Send + Sync {
    /// 适配器唯一标识
    fn tool_id(&self) -> &str;           // e.g. "blender", "godot", "vscode"
    
    /// 工具能力声明
    fn capabilities(&self) -> Vec<ToolCapability>;
    
    /// 连接/断开
    async fn connect(&mut self) -> Result<(), AdapterError>;
    async fn disconnect(&mut self) -> Result<(), AdapterError>;
    fn connection_status(&self) -> ConnectionStatus;
    
    /// 核心操作
    async fn execute(&self, command: ToolCommand) -> Result<ToolResult, AdapterError>;
    async fn query(&self, query: ToolQuery) -> Result<ToolQueryResult, AdapterError>;
    
    /// 事件流（工具端主动推送）
    fn event_stream(&self) -> Box<dyn Stream<Item = ToolEvent> + Unpin>;
    
    /// 资源同步
    async fn sync_assets(&self) -> Result<Vec<AssetDescriptor>, AdapterError>;
}
```

### 3.2 首批接入的开源软件

| 工具 | 优先级 | 接入方式 | 典型场景 |
|---|---|---|---|
| **Blender** | P0 | Python MCP Server | 3D 建模、材质编辑、动画制作 |
| **Godot** | P0 | GDScript MCP Server | 场景编辑、脚本编写、资源管理 |
| **Behy (内置)** | P0 | 内存直连 | 场景实时预览、ECS 操作 |
| **Terminal** | P1 | PTY + Command Bridge | 构建、测试、Git 操作 |
| **VS Code** | P1 | Extension API + LSP | 代码编辑、重构、调试 |
| **Figma** | P2 | REST API + Plugin | UI 设计、原型导入 |
| **Unity** | P2 | C# MCP Server | 场景编辑、资源导入 |
| **Git** | P1 | libgit2 直连 | 版本控制、分支管理 |

### 3.3 任务编排与冲突仲裁

```
用户输入 → Director AI
              │
              ▼
       ┌──────────────┐
       │  Planner      │──→ 生成多步骤计划
       └──────┬───────┘
              │
              ▼
       ┌──────────────┐
       │ Orchestrator  │──→ 将步骤分发到对应工具适配器
       │  (编排器)     │
       └──────┬───────┘
              │
    ┌─────────┼─────────┐
    ▼         ▼         ▼
 Blender    Godot     Terminal
    │         │         │
    └─────────┼─────────┘
              ▼
       ┌──────────────┐
       │ Result Merger │──→ 聚合各工具结果
       └──────┬───────┘
              │
              ▼
       ┌──────────────┐
       │ Conflict      │──→ 检测跨工具冲突（如 Blender 和 Godot
       │ Arbiter       │     同时修改同一资源）
       └──────────────┘
```

**冲突类型**：
1. **资源冲突** — 两个工具同时修改同一文件/资源
2. **依赖冲突** — 步骤 B 依赖步骤 A 的结果，但 A 在不同工具中
3. **版本冲突** — 工具的版本不兼容

**仲裁策略**：
- 乐观锁 + 版本号检查
- 最后写入者胜出（带用户确认）
- 工具间事务（跨 Blender+Godot 的 atomic 操作）

---

## 4. 黑暗玩家风格 UI 设计

### 4.1 设计语言关键词

- **深黑底色** — 不是暗灰，是近乎纯黑的 `#0a0a0c` 基调
- **霓虹强调色** — 品红 `#ff2d78`、青蓝 `#00e5ff`、电紫 `#b44dff`
- **扫描线/CRT 质感** — HUD 风格边框、数据流动线条
- **终端美学** — 等宽字体、半透明面板、扫描线叠加层
- **赛博朋克工业感** — 科技 panel 边框、发光文字、噪点纹理

### 4.2 配色方案

```
┌──────────────────────────────────────────────────────────┐
│  WindWave Color System — Cyber Dark Theme                │
├─────────────┬──────────┬─────────────────────────────────┤
│ 用途         │ 色值      │ 说明                             │
├─────────────┼──────────┼─────────────────────────────────┤
│ 主背景       │ #0a0a0c  │ 最深黑，Viewport 背景              │
│ 面板背景     │ #121218  │ 深黑蓝，面板底色                   │
│ 次级面板     │ #1a1a24  │ 卡片、分组背景                     │
│ 边框         │ #2a2a3a  │ 默认边框                          │
│ 激活边框     │ #00e5ff  │ 选中/高亮边框（青蓝霓虹）           │
│ 主强调色     │ #ff2d78  │ 危险/重要操作（品红霓虹）           │
│ 次强调色     │ #b44dff  │ 信息/次要操作（电紫）               │
│ 成功色       │ #00ff88  │ 完成/成功状态                      │
│ 警告色       │ #ffb800  │ 警告状态                          │
│ 文字主色     │ #e0e0e0  │ 主要文字                          │
│ 文字次色     │ #888899  │ 次要文字/注释                     │
│ 文字浅色     │ #555566  │ 禁用/占位文字                      │
├─────────────┴──────────┴─────────────────────────────────┤
│  Agent 角色色:                                             │
│  Director   │ #ff2d78  │ 品红 — 决策者                     │
│  Artist     │ #00e5ff  │ 青蓝 — 创作者                     │
│  Engineer   │ #b44dff  │ 电紫 — 实现者                     │
│  QA         │ #00ff88  │ 翠绿 — 验证者                     │
│  System     │ #ffb800  │ 琥珀 — 系统消息                    │
└─────────────┴──────────┴─────────────────────────────────┘
```

### 4.3 UI 组件风格规范

#### 面板（Panels）
```
┌─ ╔══════════════════════╗ ─┐
│  ║  DIRECTOR DECK       ║  │  ← 双层边框 + 发光标题
│  ╚══════════════════════╝  │
│                             │
│  ┌─────────────────────┐   │  ← 内面板圆角 + 微边框
│  │ Tool Status         │   │
│  │  ■ Blender  ONLINE  │   │  ← 状态指示灯（像素风圆点）
│  │  ■ Godot   STANDBY  │   │
│  │  ■ Terminal ACTIVE  │   │
│  └─────────────────────┘   │
└─────────────────────────────┘
```

#### 按钮
- 默认：透明底 + `#2a2a3a` 边框，hover 时边框变为主强调色
- 主按钮：主强调色背景 + 深色文字
- 危险按钮：红色霓虹边框 + hover 时背景渐变

#### 输入框
- 暗底 + 底部单线（underline），focus 时底部线变为霓虹青蓝
- 光标：闪烁竖线，青蓝色

#### 滚动条
- 极细（4px）+ 半透明 `#2a2a3a`，hover 变亮

#### 字体
- UI 文字：`JetBrains Mono`（等宽，终端感）
- 标题：`Orbitron`（科技感几何字体）
- 正文：`Inter`（可读性）

### 4.4 新增/改造的 UI 面板

| 面板名称 | 用途 | 新增/改造 |
|---|---|---|
| **Command Deck** | 中央命令输入，类似终端 + 命令面板混合 | 改造 |
| **Tool Dock** | 左侧工具链状态面板，显示所有接入工具 | 新增 |
| **Live Viewport** | 多窗口实时预览（分屏显示 Blender/Godot 画面） | 改造 |
| **Agent Pipeline** | 可视化 Agent 任务流水线（节点图风格） | 新增 |
| **Resource Matrix** | 跨工具资源清单、冲突提示 | 新增 |
| **HUD Overlay** | 帧率、内存、Agent 状态悬浮 HUD | 新增 |

---

## 5. 实施路线图

### Phase 1: 基础设施（v0.3.0）

- [ ] 定义 `ToolAdapter` trait 和 `ToolCommand`/`ToolResult` 协议
- [ ] 实现 `ToolOrchestrator` 核心编排引擎
- [ ] 实现 `ConflictArbiter` 冲突检测框架
- [ ] 终端适配器（Terminal Adapter）——第一个外部工具适配器
- [ ] UI 暗黑主题系统重构（css/theme 变量化）

### Phase 2: 核心工具接入（v0.4.0）

- [ ] Blender MCP Adapter — Python 脚本桥接
- [ ] Godot MCP Adapter — GDScript 桥接
- [ ] Tool Dock 面板实现
- [ ] Agent Pipeline 可视化面板
- [ ] 跨工具 undo/redo

### Phase 3: 深度集成（v0.5.0）

- [ ] VS Code Extension Adapter
- [ ] Figma Plugin Adapter
- [ ] Resource Matrix 面板
- [ ] HUD Overlay 系统
- [ ] 工具间事务支持

### Phase 4: 生态扩展（v0.6.0+）

- [ ] 第三方适配器 SDK
- [ ] 社区适配器市场
- [ ] Unity Adapter
- [ ] AI 驱动的工具选择与编排优化

---

## 6. 与现有架构的关系

| 现有模块 | 改造方向 |
|---|---|
| `agent-core` | 保持不变，Director/Planner 逻辑可直接复用 |
| `bevy-adapter` | 降级为"内置 Bevy 工具适配器"，与其他适配器平级 |
| `agent-ui` | 全面改造为黑暗玩家风格，新增 Tool Dock、Pipeline 面板 |
| `multica-bridge` | 可作为"外部协作适配器"保留，或融入 Tool Dock |
| `ai-frameworks` | 作为可选的 AI 后端插件，通过适配器协议接入 |

---

## 7. 风险与缓解

| 风险 | 影响 | 缓解措施 |
|---|---|---|
| 多工具进程管理复杂度 | 高 | 每个适配器独立进程，崩溃隔离；supervisor 进程守护 |
| 性能开销（多进程 IPC） | 中 | 懒连接、按需启动；批量化命令合并 |
| 工具 API 不兼容 | 中 | MCP 协议作为标准桥接层；提供 fallback 文件同步模式 |
| UI 复杂度膨胀 | 中 | 默认折叠非活跃工具；可配置面板可见性 |
| 用户学习曲线 | 低 | Command Palette 统一入口；自然语言驱动的工具选择 |

---

## 8. 场景预演 —— 端到端工作流模拟

### 8.1 场景 A：创建 3D 角色并导入游戏

**用户输入**（在 Command Deck）：
> "创建一个 low-poly 骑士角色，带行走动画，放到当前场景中"

**完整执行流程**：

```
时间轴 | 组件             | 动作
─────────────────────────────────────────────────────────────────
T+0s   | Command Deck     | 用户输入 → KeywordMatcher 路由 → Director
T+0.1s | Director.Planner | 生成计划:
       |                  |   Step 1: [Blender] 创建角色模型
       |                  |   Step 2: [Blender] 绑定骨骼 + 行走动画
       |                  |   Step 3: [Blender] 导出 FBX → assets/models/knight.fbx
       |                  |   Step 4: [Godot] 导入 FBX 到场景
       |                  |   Step 5: [Bevy] 在编辑器中放置角色
T+0.2s | Orchestrator     | 路由 Step 1 → BlenderAdapter
       |                  | 检测 Blender 未运行 → 自动启动 blender --background
       |                  | 等待 BlenderAdapter 报告 ONLINE
T+0.5s | BlenderAdapter   | 连接成功。发送 MCP 命令:
       |                  |   tools/call: create_mesh { name: "knight", type: "humanoid" }
       |                  | Blender 执行 → 创建基础人形网格
T+2s   | BlenderAdapter   | 发送 Sculpt 命令细化盔甲细节
       |                  |   tools/call: sculpt_detail { target: "knight", style: "armor" }
T+5s   | BlenderAdapter   | Step 2: 绑定骨骼
       |                  |   tools/call: add_armature { target: "knight", type: "humanoid" }
       |                  |   tools/call: create_animation { name: "walk", frames: 24 }
T+12s  | BlenderAdapter   | Step 3: 导出 FBX
       |                  |   tools/call: export_fbx { target: "knight", path: "..." }
       |                  | → event_stream 推送: { event: "export_complete", file: "knight.fbx" }
T+12s  | ResultMerger     | 聚合 Blender 结果: { success: true, assets: ["knight.fbx"] }
T+12s  | Orchestrator     | 路由 Step 4 → GodotAdapter
       |                  | Godot 已在运行 → 复用现有连接
T+12.5s| GodotAdapter     | 发送命令:
       |                  |   tools/call: import_asset { path: "knight.fbx" }
       |                  |   tools/call: create_animated_sprite { asset: "knight.fbx" }
T+14s  | GodotAdapter     | 完成。event_stream 推送场景更新
T+14s  | ResultMerger     | 聚合 Godot 结果
T+14s  | Orchestrator     | 路由 Step 5 → BevyAdapter (内存直连)
T+14.1s| BevyAdapter      | 执行 EngineCommand::SpawnPrefab { asset_handle: "knight.fbx" }
       |                  | → 角色出现在 Scene Viewport 中
T+14.2s| Pipeline          | 状态: COMPLETED
       |                  | Agent Pipeline 面板显示: 5/5 步骤完成
T+14.3s| Command Deck     | 显示结果: "骑士角色已创建并放置到场景中 ✅"
       |                  | 3 个工具协作: Blender → Godot → Bevy
```

**关键观察**：
1. 总耗时 ~14 秒，大部分时间花在 Blender 建模
2. 每个步骤间有 **隐式依赖**：Step 4 必须在 Step 3 完成后才能执行
3. Orchestrator 需要 **等待 + 确认** 模式：不能 fire-and-forget
4. Blender 的 `--background` 模式无 GUI，但建模操作仍能执行

**可能出错的点**：
- Blender 启动失败 → Orchestrator 重试 3 次 → 失败后提示用户手动启动
- FBX 导出路径不存在 → BlenderAdapter 自动创建目录
- Godot 未运行 → Orchestrator 自动启动或降级为直接文件操作

---

### 8.2 场景 B：调试运行时崩溃

**用户输入**：
> "游戏在加载关卡 3 时崩溃了，帮我找到原因"

**执行流程**：

```
T+0s   | Command Deck     | 输入 → Director
T+0.1s | Director.Planner | 生成调试计划:
       |                  |   Step 1: [Terminal] 运行游戏，捕获崩溃日志
       |                  |   Step 2: [Terminal] 分析堆栈跟踪
       |                  |   Step 3: [VS Code] 定位崩溃代码位置
       |                  |   Step 4: [Bevy] 检查崩溃时的场景状态
       |                  |   Step 5: [VS Code] 生成修复建议
T+0.2s | Orchestrator     | 路由 Step 1 → TerminalAdapter
T+0.3s | TerminalAdapter  | 执行: cargo run -- --level 3 2>&1
       |                  | PTY 输出流实时显示在 Command Deck 底部
T+3s   | TerminalAdapter  | 崩溃！捕获到:
       |                  |   thread 'main' panicked at src/level_loader.rs:142:39:
       |                  |   index out of bounds: the len is 0 but the index is 0
T+3.1s | TerminalAdapter  | event_stream 推送:
       |                  |   { event: "process_exited", code: 101, output: "..." }
T+3.2s | ResultMerger     | 提取关键信息: "level_loader.rs:142", "index out of bounds"
T+3.3s | Orchestrator     | 路由 Step 3 → VSCodeAdapter
T+3.5s | VSCodeAdapter    | 发送命令:
       |                  |   tools/call: open_file { path: "src/level_loader.rs", line: 142 }
       |                  |   tools/call: get_context { path: "src/level_loader.rs", around: 142 }
       |                  | 返回代码上下文:
       |                  |   L138: let data = load_level_data(level_id)?;
       |                  |   L139: let tiles = data.tiles;  // ← 空 Vec
       |                  |   L140: for i in 0..tiles.len() {
       |                  |   L141:     let tile = tiles[i];  // ← 这里不会 panic
       |                  |   L142:     let first = tiles[0]; // ← PANIC HERE
T+4s   | Orchestrator     | 路由 Step 4 → BevyAdapter
T+4.1s | BevyAdapter      | query 场景状态:
       |                  |   关卡 3 的 tiles 数据为空 → 数据文件可能损坏
T+4.5s | Orchestrator     | 路由 Step 5 → VSCodeAdapter
T+5s   | VSCodeAdapter    | 生成修复:
       |                  |   建议: 在 L138 后添加空检查
       |                  |   if data.tiles.is_empty() {
       |                  |       return Err("Level 3 has no tiles");
       |                  |   }
T+5.2s | Command Deck     | 显示完整诊断报告:
       |                  |   🔴 ROOT CAUSE: 关卡 3 数据文件为空
       |                  |   📍 LOCATION: src/level_loader.rs:142
       |                  |   🔧 FIX: 添加空数据检查
       |                  |   [Apply Fix] [Show Diff] [Ignore]
```

**关键观察**：
1. 这是一个 **串行调试流程**，每步依赖前一步的结果
2. TerminalAdapter 的 PTY 输出需要实时流式显示（用户想看到编译过程）
3. VS Code 和 Terminal 可能同时操作同一个文件（需要文件锁协调）
4. 用户需要在最后一步确认是否应用修复（approval gate）

**与现有代码的对比**：
- 当前 [handle_agent_input](file:///Users/chengyongwei/Library/Mobile Documents/com~apple~CloudDocs/gameedit/风浪/src/main.rs#L204) 只是把用户输入传给 Director，然后等待 `drain_bridge_commands()`
- 改造后，Director 不再直接产出 `EngineCommand`，而是产出 `Plan` 交给 Orchestrator
- `drain_bridge_commands()` 变成 `drain_plan_steps()`，由 Orchestrator 逐个分发

---

### 8.3 场景 C：跨工具并发操作 + 冲突

**用户输入**：
> "优化玩家角色的材质，同时编译代码"

**执行流程**：

```
T+0s   | Orchestrator     | Planner 识别: Step 1 和 Step 2 无依赖 → 可并发
       |                  |   并行组 A: [Blender] 优化材质
       |                  |   并行组 B: [Terminal] cargo build
       |                  |   等待组:   [Bevy] 热重载新材质 + 新二进制
T+0.1s | BlenderAdapter   | 开始执行: 打开 player.blend → 调整材质节点
T+0.1s | TerminalAdapter  | 开始执行: cargo build --release
T+5s   | TerminalAdapter  | 编译完成。event_stream: { event: "build_success" }
T+8s   | BlenderAdapter   | 材质优化完成。导出 player_optimized.fbx
T+8s   | ⚠️ ConflictArbiter| 检测到冲突:
       |                  |   Blender 要写入 player.fbx
       |                  |   Terminal 编译产物引用了旧的 player.fbx
       |                  | → 仲裁: Blender 先完成 → 写入 → Terminal 增量编译
T+8.1s | Orchestrator     | 路由到 BevyAdapter: 热重载
T+8.5s | BevyAdapter      | 加载新 player.fbx + 新二进制
T+8.6s | Command Deck     | 显示: "材质优化完成 ✅ | 编译成功 ✅"
```

**冲突场景详解**：

| 冲突类型 | 触发条件 | 检测方式 | 解决方式 |
|---|---|---|---|
| 文件写入冲突 | Blender 和 Terminal 同时写入同一文件 | 文件锁 + 版本号对比 | 排队执行 |
| 资源依赖冲突 | Blender 修改了 Terminal 编译引用的资源 | AssetDescriptor 依赖图 | 提示用户确认 |
| 状态不一致 | Bevy 场景中的实体被 Blender 删除 | 实体 ID 映射表比对 | 自动同步或警告 |

**Orchestrator 的并发调度策略**：

```
┌─────────────────────────────────────────────────────────┐
│                  Orchestrator 调度器                      │
│                                                         │
│  输入: Plan { steps: [A, B, C, D, E] }                  │
│                                                         │
│  1. 构建依赖图:                                          │
│     A ──→ C ──→ E                                       │
│     B ──→ D ──→ E                                       │
│                                                         │
│  2. 拓扑排序:                                            │
│     Wave 1: [A, B]   ← 并发执行                         │
│     Wave 2: [C, D]   ← 并发执行 (A,B 都完成后)           │
│     Wave 3: [E]      ← C,D 都完成后                      │
│                                                         │
│  3. 每个 Wave 内部:                                      │
│     - 检查资源冲突（ConflictArbiter）                     │
│     - 分配适配器                                          │
│     - 并行 execute()                                     │
│     - await 所有结果                                     │
│                                                         │
│  4. 异常处理:                                            │
│     - 单个步骤失败 → 重试 3 次 → 标记失败                 │
│     - 依赖失败的步骤 → 跳过                               │
│     - 全部失败 → 回滚到 Plan 前的快照                     │
└─────────────────────────────────────────────────────────┘
```

---

### 8.4 场景 D：Figma 设计稿 → Godot UI 自动化

**用户输入**：
> "把 Figma 里的主菜单设计稿导入 Godot 做成 UI"

**执行流程**：

```
T+0s   | Director.Planner | 计划:
       |                  |   Step 1: [Figma] 导出设计稿为结构化 JSON
       |                  |   Step 2: [Figma] 导出切图资源
       |                  |   Step 3: [Godot] 根据 JSON 创建 UI 场景
       |                  |   Step 4: [Godot] 绑定按钮事件
T+0.2s | FigmaAdapter     | REST API 调用:
       |                  |   GET /v1/files/{file_key}/nodes?ids=main-menu
       |                  |   → 返回节点树: Frame > Button > Text 等
       |                  |   → 转换为内部 UI 描述格式
T+1s   | FigmaAdapter     | 导出图片:
       |                  |   GET /v1/images/{file_key}?ids=bg,btn_normal,btn_hover
       |                  |   → 下载 PNG → 保存到 assets/ui/
T+3s   | ResourceMerger   | 聚合: JSON 布局 + 图片资源
T+3.1s | GodotAdapter     | 创建 Godot UI 场景:
       |                  |   tools/call: create_control { type: "Panel", name: "MainMenu" }
       |                  |   tools/call: create_button { text: "Start", pos: [...] }
       |                  |   tools/call: set_texture { target: "btn_start", texture: "..." }
       |                  |   tools/call: connect_signal { signal: "pressed", handler: "..." }
T+5s   | Command Deck     | 显示: "主菜单 UI 已创建 ✅"
       |                  | Live Viewport 显示 Godot 场景预览
```

**Figma 适配器的关键限制**：
1. Figma REST API 有速率限制 → 需要缓存
2. Figma 的设计 Token（颜色、字体、间距）需要映射到 Godot 的资源系统
3. Auto Layout 的嵌套结构不能 1:1 映射到 Godot Control 节点 → 需要转换层

**简化方案（Phase 3 前）**：
- 不直接调用 Figma API
- 而是让用户先导出 Figma JSON + 图片到本地文件夹
- FigmaAdapter 简化为读取本地文件 + 生成 Godot 场景

---

## 9. 深度评估

### 9.1 性能评估

**延迟预算分析**（从用户输入到第一个可见结果）：

| 阶段 | 最理想 | 典型 | 最坏 | 备注 |
|---|---|---|---|---|
| LLM 推理（Planner） | 500ms | 2s | 8s | 取决于 LLM 提供商和 token 量 |
| Plan 解析 + 路由 | 10ms | 20ms | 50ms | 纯本地计算 |
| 工具启动（冷启动） | 0ms | 2s | 10s | Blender 冷启动约 3-5s |
| 命令执行 | 10ms | 500ms | 30s | 取决于操作复杂度 |
| 结果聚合 + UI 渲染 | 5ms | 10ms | 20ms | 本地 |
| **总计（首字节）** | **~525ms** | **~4.5s** | **~38s** | |

**IPC 开销基准测试**（估算）：

| 通信方式 | 单次 RTT | 适用场景 |
|---|---|---|
| 内存直连（BevyAdapter） | < 1ms | 内置 Bevy 操作 |
| stdio JSON-RPC（Blender） | 1-5ms | 本地工具进程 |
| TCP localhost（Godot） | 0.5-2ms | 本地工具进程 |
| WebSocket（Multica） | 10-50ms | 远程协作 |
| REST API（Figma） | 100-500ms | 云端服务 |

**结论**：本地工具（Blender/Godot/Terminal）的 IPC 开销在 1-5ms 级别，用户感知不到延迟。真正的瓶颈是 LLM 推理和工具冷启动。

**内存占用估算**：

| 组件 | 内存 | 备注 |
|---|---|---|
| WindWave 编辑器 | ~200MB | 当前基准 |
| Blender 进程 | ~500MB-2GB | 取决于场景复杂度 |
| Godot 进程 | ~300MB-1GB | 取决于场景复杂度 |
| Termina 进程 | ~10MB | 轻量 |
| VS Code 进程 | ~300MB | 含扩展 |
| **总计** | **~1.3GB-3.5GB** | 需要 16GB RAM 机器 |

---

### 9.2 故障模式分析

**FMEA（故障模式与影响分析）**：

| # | 故障模式 | 触发条件 | 影响 | 严重度 | 检测方式 | 缓解措施 |
|---|---|---|---|---|---|---|
| F1 | 适配器进程崩溃 | Blender segmentation fault | 当前步骤失败，后续依赖步骤阻塞 | **高** | 进程监控 + 心跳 | 自动重启 + 重试；降级到手动模式 |
| F2 | 适配器无响应 | 工具执行死循环 | Orchestrator 永久等待 | **高** | 超时机制（30s） | 超时后 kill 进程 + 标记失败 |
| F3 | 命令执行结果不一致 | Blender 返回成功但文件未写入 | 后续步骤拿到错误文件 | **高** | 执行后 verify（检查文件存在） | 重试 + 告警 |
| F4 | 资源冲突未检测 | 两个工具同时写同一文件 | 文件损坏 | **中** | 文件锁 + 版本号 | 排队执行 |
| F5 | 僵尸进程 | 适配器 disconnect 失败，进程残留 | 内存泄漏 | **中** | 进程树监控 | 定期清理孤儿进程 |
| F6 | LLM 规划错误 | Planner 生成不可执行的步骤 | 整个 Plan 失败 | **中** | 步骤前置校验 | 允许用户编辑 Plan |
| F7 | 工具版本不兼容 | Blender 3.6 不支持 4.2 的 API | 命令执行失败 | **中** | capabilities() 版本检查 | 提示升级工具或降级命令 |
| F8 | 多工具状态不一致 | Godot 认为某实体存在，Bevy 已删除 | 操作冲突 | **低** | 定期全量同步 | 以 Bevy 为准（source of truth） |
| F9 | 用户手动操作干扰 | 用户在 Blender GUI 中手动修改 | Agent 操作基于过期状态 | **低** | event_stream 监听外部变更 | 提示用户"外部修改检测到" |

**故障恢复流程**：

```
┌──────────────┐
│ 检测到故障    │
│ (F1-F9)      │
└──────┬───────┘
       │
       ▼
┌──────────────┐     ┌─────────────────┐
│ 可自动恢复？  │──是──→│ 重试 (最多3次)   │
└──────┬───────┘     └────────┬────────┘
       │ 否                   │
       ▼                      ▼
┌──────────────┐     ┌─────────────────┐
│ 通知用户     │     │ 成功？           │
│ - 故障描述   │     └────────┬────────┘
│ - 建议操作   │              │ 是         │ 否
└──────┬───────┘              ▼            ▼
       │              ┌──────────┐  ┌──────────┐
       ▼              │ 继续执行 │  │ 标记失败 │
┌──────────────┐      └──────────┘  └────┬─────┘
│ 用户选择:    │                         │
│ [重试] [跳过]│                         ▼
│ [手动修复]   │               ┌──────────────┐
│ [取消计划]   │               │ 跳过依赖步骤 │
└──────────────┘               └──────────────┘
```

---

### 9.3 替代架构方案对比

**方案 A：当前设计（MCP 桥接）**

```
优点: 标准化协议、社区支持、工具独立进程
缺点: 每个工具需要单独实现 MCP Server、IPC 开销
```

**方案 B：直接 SDK 嵌入（将 Blender Python 等直接编译进 Rust）**

```
优点: 零 IPC 开销、类型安全
缺点: 无法实现（Blender Python API 必须在 Blender 进程内运行）
结论: 不可行
```

**方案 C：文件系统桥接（所有操作通过读写文件）**

```
优点: 最简单、任何工具都支持
缺点: 实时性差、冲突检测困难、无法获取工具内部状态
适用: 作为 Fallback 模式
```

**方案 D：VNC/屏幕共享操控（直接操作工具 GUI）**

```
优点: 不需要任何适配器，支持所有工具
缺点: 极其脆弱（像素级操作）、无法获取结构化数据、速度慢
结论: 仅在 AI 视觉理解场景下作为补充
```

**方案 E：统一资源总线（类似 ROS 的消息总线）**

```
优点: 工具间松耦合、发布/订阅模式
缺点: 引入额外中间件、增加复杂度
适用: 如果未来工具数量 > 10 个
```

**推荐路径**：**方案 A（MCP）作为主路径 + 方案 C（文件同步）作为 Fallback**。

---

### 9.4 迁移路径分析

**从当前架构到目标架构的逐步迁移**：

```
当前状态 (v0.2.x)
  │
  │  Phase 1: 不动现有代码，新增 crate
  │
  ├── 新建 crates/tool-hub/
  │   ├── src/lib.rs           # ToolAdapter trait + ToolOrchestrator
  │   ├── src/adapters/
  │   │   ├── mod.rs
  │   │   ├── terminal.rs      # TerminalAdapter
  │   │   └── bevy_adapter.rs  # Bevy 实现 ToolAdapter
  │   ├── src/orchestrator.rs  # 编排引擎
  │   └── src/conflict.rs      # 冲突仲裁
  │
  │  Phase 2: 桥接旧架构
  │
  ├── 修改 src/main.rs:
  │   - handle_agent_input 中的 drain_bridge_commands()
  │     改为 drain_plan_steps()
  │   - Plan 步骤不再直接变成 EngineCommand
  │     而是通过 ToolOrchestrator 分发
  │
  ├── 修改 bevy-adapter:
  │   - 实现 ToolAdapter trait
  │   - EngineCommand 作为 ToolCommand::Bevy 的子类型
  │
  │  Phase 3: 逐步替换
  │
  ├── 移除 PendingCommands 资源（被 Orchestrator 取代）
  ├── 移除 CommandProcessorPlugin（被 Orchestrator 调度取代）
  ├── multica-bridge 降级为远程适配器
  │
  └── 目标状态 (v0.5.0)
```

**向后兼容策略**：
- Phase 1-2 期间，`PendingCommands` 和 `ToolOrchestrator` 并存
- 优先使用 `ToolOrchestrator` 路径，Fallback 到旧路径
- Phase 3 切换完成后，移除旧路径

**回滚策略**：
- 每个 Phase 都是可独立回滚的
- 核心抽象（`ToolAdapter` trait）在 Phase 1 就确定，后续不会大改

---

### 9.5 安全评估

**信任边界**：

```
┌──────────────────────────────────────────────┐
│  WindWave 进程 (信任)                          │
│  ┌──────────────────────────────────────┐    │
│  │  Orchestrator + Core                  │    │
│  └──────────────────────────────────────┘    │
│                     │                         │
│              ┌──────┴──────┐                  │
│              │  IPC 边界    │                  │
│              └──────┬──────┘                  │
└─────────────────────┼─────────────────────────┘
                      │
    ┌─────────┬───────┼───────┬─────────┐
    ▼         ▼       ▼       ▼         ▼
┌───────┐ ┌───────┐ ┌───────┐ ┌───────┐ ┌───────┐
│Blender│ │Godot  │ │VS Code│ │Terminal│ │Figma │
│(半信任)│ │(半信任)│ │(半信任)│ │(信任) │ │(不信任)│
└───────┘ └───────┘ └───────┘ └───────┘ └───────┘
```

**风险点**：

| 工具 | 信任级别 | 风险 | 缓解 |
|---|---|---|---|
| Terminal | 信任 | 注入恶意命令 | 命令白名单 + 沙箱 |
| Blender | 半信任 | 恶意 Python 脚本 | 插件签名验证 |
| Figma | 不信任 | API Key 泄露 | Token 加密存储 + 用户确认每次操作 |
| VS Code | 半信任 | 恶意扩展 | 扩展白名单 |

**命令注入防护**：
```rust
// TerminalAdapter 的命令白名单
const ALLOWED_COMMANDS: &[&str] = &[
    "cargo", "git", "rustc", "rustfmt", "clippy",
    "pnpm", "npm", "node",
    "ls", "cat", "head", "tail", "wc", "grep",
];

fn sanitize_command(cmd: &str) -> Result<String, AdapterError> {
    let base = cmd.split_whitespace().next().unwrap_or("");
    if !ALLOWED_COMMANDS.contains(&base) {
        return Err(AdapterError::Security(format!(
            "Command '{}' is not in the allowed list", base
        )));
    }
    // 禁止 shell 注入字符
    if cmd.contains(';') || cmd.contains('|') || cmd.contains('`') || cmd.contains("$(") {
        return Err(AdapterError::Security("Command contains forbidden characters".into()));
    }
    Ok(cmd.to_string())
}
```

---

## 10. 总结：关键决策清单

| # | 决策点 | 选项 | 推荐 | 理由 |
|---|---|---|---|---|
| 1 | ToolCommand 类型 | A) 动态 JSON B) 泛型枚举 | **B** | 编译时类型安全，WindWave 是单体应用 |
| 2 | 适配器通信协议 | A) MCP B) 自定义 | **A** | 标准化，社区支持 |
| 3 | 适配器进程模型 | A) 进程内 B) 独立进程 | **B** | 崩溃隔离 |
| 4 | 并发调度 | A) 串行 B) 按依赖图并行 | **B** | 明显提升效率 |
| 5 | Fallback 模式 | A) 文件同步 B) 无 | **A** | 工具不支持 MCP 时的兜底 |
| 6 | 冲突仲裁 | A) 自动 B) 用户确认 | **B (默认)** | 安全优先，可配置 |
| 7 | 命令安全 | A) 白名单 B) 沙箱 | **A (Phase 1)** | 简单有效 |
| 8 | 旧架构兼容 | A) 立即替换 B) 逐步迁移 | **B** | 降低风险 |
| 9 | 多工具状态 source of truth | A) Bevy B) 文件系统 | **A** | Bevy 是编辑器核心 |
| 10 | 第三方适配器 SDK | A) Rust B) 多语言 | **A (Phase 3)** | 先内部验证，再开放 |

---

## 11. 设计遗漏分析 —— 20 个待完善点

> 通过系统性模拟推演，识别出以下设计文档尚未覆盖的关键领域。

### 11.1 严重遗漏（阻塞实现）

#### G1: Plan 数据结构未定义

**问题**：文档反复提到 Planner 生成 Plan、Orchestrator 执行 Plan，但 Plan/PlanStep 的数据结构从未定义。

**影响**：这是整个 Orchestrator 的核心数据结构，不定义就无法开始编码。

**需要明确**：
```rust
/// 一个步骤的完整描述
struct PlanStep {
    id: StepId,
    title: String,                    // 人类可读标题
    description: String,              // 详细描述（给 LLM 的 prompt）
    target_tool: ToolId,              // 目标工具（或 "auto" 让 Orchestrator 选择）
    command: ToolCommand,             // 要执行的命令（JSON 或枚举）
    dependencies: Vec<StepId>,        // 前置依赖步骤
    expected_outputs: Vec<OutputDescriptor>, // 预期的输出资源
    timeout: Duration,                // 超时时间
    retry_policy: RetryPolicy,        // 重试策略
    approval_required: bool,          // 是否需要用户确认
}

struct Plan {
    id: PlanId,
    goal: String,                     // 用户的原始意图
    steps: Vec<PlanStep>,
    mode: ExecutionMode,              // Sequential / Parallel / Hybrid
    created_at: Instant,
    context: PlanContext,             // 来自 Planner 的上下文
}
```

#### G2: 步骤间数据传递机制缺失

**问题**：场景 A 中 Step 3（Blender 导出 FBX）→ Step 4（Godot 导入 FBX），Step 4 需要知道 FBX 的路径。这个路径怎么从 Step 3 传到 Step 4？

**影响**：没有数据传递机制，步骤之间就是孤立的，无法形成真正的流水线。

**方案**：
```rust
struct PlanContext {
    // 步骤输出注册表，key = "step_id.output_name"
    outputs: HashMap<String, OutputValue>,
}

enum OutputValue {
    FilePath(PathBuf),
    EntityId(u64),
    Text(String),
    AssetList(Vec<AssetDescriptor>),
    Json(serde_json::Value),
}

// Planner 在生成 Plan 时指定：
//   Step 3: expected_outputs = [OutputDescriptor { name: "fbx_path", type: FilePath }]
//   Step 4: 引用 Step 3 的输出
//     command.inputs = {"fbx_path": "$step_3.fbx_path"}
```

#### G3: Orchestrator 对 Bevy 的异步适配问题

**问题**：Bevy 是同步 ECS 循环，`ToolAdapter::execute()` 是 async。BevyAdapter 不能真正 async——它必须在 Bevy 的 `Update` 阶段同步执行命令。

**影响**：核心 trait 对 Bevy 不适用，需要特殊处理。

**方案**：
```rust
// BevyAdapter 不实现标准的 async execute()，而是有专门的同步通道
impl BevyAdapter {
    fn enqueue_commands(&self, commands: Vec<EngineCommand>) {
        // 写入 PendingCommands 资源，Bevy 在下一帧处理
    }
    
    fn wait_for_result(&self) -> EngineCommandResult {
        // 轮询直到命令执行完成
        // 或用 oneshot channel 通知
    }
}

// Orchestrator 中特殊处理
match step.target_tool {
    "bevy" => {
        // 同步路径
        bevy_adapter.enqueue_commands(cmds);
        loop {
            app.update(); // 驱动 Bevy 一帧
            if let Some(result) = bevy_adapter.try_get_result() {
                break result;
            }
        }
    }
    _ => {
        // 标准 async 路径
        adapter.execute(step.command).await
    }
}
```

#### G4: 用户中断/取消机制缺失

**问题**：一个跨工具 Plan 正在执行（比如 Blender 渲染需要 30 秒），用户突然想取消。怎么优雅地中止？

**影响**：用户体验极差——用户只能等或者强制 kill 进程。

**需要设计**：
- `CancellationToken` 传递到每个 `execute()` 调用
- 定期检查 cancellation（在每个步骤开始前）
- 取消后的清理：已修改的文件是否回滚？已启动的进程是否 kill？
- UI 上的取消按钮 + 确认对话框

#### G5: 工具发现与配置系统缺失

**问题**：文档假设工具已经在运行或可以自动启动，但从不讨论：用户怎么告诉 WindWave "我的 Blender 安装在这里"？怎么配置 Figma API Key？

**影响**：用户无法实际使用任何外部工具。

**需要设计**：
```rust
struct ToolRegistry {
    adapters: HashMap<ToolId, Box<dyn ToolAdapter>>,
    configs: HashMap<ToolId, ToolConfig>,
}

struct ToolConfig {
    tool_id: ToolId,
    enabled: bool,                    // 用户是否启用此工具
    executable_path: Option<PathBuf>, // 工具的可执行文件路径
    auto_start: bool,                 // 是否自动启动
    mcp_server_path: Option<PathBuf>, // MCP Server 脚本路径
    credentials: Option<Credentials>, // API Key 等
    working_directory: Option<PathBuf>,
    custom_env: HashMap<String, String>,
}

// 自动发现已安装的工具
fn discover_installed_tools() -> Vec<ToolDiscovery> {
    // 检查 PATH 中是否有 blender, godot, code 等
    // 检查常见安装路径
}
```

---

### 11.2 中等遗漏（影响质量）

#### G6: 进度报告与状态反馈模型

**问题**：场景 A 中 Blender 建模 12 秒，用户看到什么？当前只有开始和结束两个状态，中间 12 秒是黑盒。

**影响**：用户不知道 Agent 是在工作还是卡死了。

**需要**：
```rust
enum PlanProgress {
    Planning,                          // LLM 正在生成计划
    ToolStarting { tool: ToolId },     // 正在启动工具
    Executing { step: StepId, progress: f32, message: String }, // 执行中
    StepCompleted { step: StepId },    // 步骤完成
    PlanCompleted,                     // 全部完成
    Failed { step: StepId, error: String },
}

// 每个适配器在执行过程中可以推送进度
// event_stream 中增加 ProgressEvent 类型
```

#### G7: Plan 可编辑性与模板

**问题**：LLM 生成的 Plan 不一定完美。用户能不能在 Plan 执行前编辑它？能不能保存常用 Plan 作为模板？

**影响**：没有这个能力，用户对 Agent 的控制力很弱。

**需要**：
- Agent Pipeline 面板支持拖拽编辑步骤
- 步骤可以增删改、重排序
- 执行前预览 Plan（纯文本 + 可视化）
- 保存/加载 Plan 模板（JSON 文件）
- "收藏"常用 Plan

#### G8: 跨工具 undo/redo 详细设计

**问题**：Phase 2 提到"跨工具 undo/redo"，但完全没有设计。Blender 的操作怎么 undo？Godot 的怎么 undo？它们如何协同？

**影响**：这是 Phase 2 的核心功能，但完全没有设计细节。

**方案**：
```rust
struct CrossToolUndoEntry {
    plan_id: PlanId,
    step_id: StepId,
    tool: ToolId,
    // 每个工具负责自己的逆向操作
    reverse_command: ToolCommand,
    // 资源快照（文件在操作前的内容）
    file_snapshots: Vec<FileSnapshot>,
}

struct CrossToolUndoHistory {
    entries: Vec<CrossToolUndoEntry>,
    position: usize, // 当前位置（支持 redo）
}

// undo 时：按逆序执行每个 entry 的 reverse_command
// 如果某工具不支持 reverse_command，则恢复 file_snapshots
```

#### G9: 工具能力语义化与 Planner 集成

**问题**：`capabilities()` 返回 `Vec<ToolCapability>`，但 Planner（LLM）怎么理解这些能力？需要把能力描述注入到 LLM prompt 中。

**影响**：Planner 不知道每个工具能做什么，无法生成正确的 Plan。

**需要**：
```rust
struct ToolCapability {
    name: String,              // "create_3d_mesh"
    description: String,       // "Create a 3D mesh in Blender with given parameters"
    parameters: Vec<ParameterDef>,
    // LLM prompt 生成
    fn to_llm_description(&self) -> String {
        format!("- {}: {}", self.name, self.description)
    }
}

// Orchestrator 在调用 Planner 前，将所有工具的 capabilities
// 注入到 system prompt 中
fn build_planner_prompt(tools: &[ToolCapability]) -> String {
    let mut prompt = String::from("You have access to the following tools:\n");
    for cap in tools {
        prompt.push_str(&cap.to_llm_description());
        prompt.push('\n');
    }
    prompt
}
```

#### G10: Live Viewport 技术实现

**问题**：文档提到"Live Viewport 分屏显示 Blender/Godot 画面"，但完全没有技术方案。

**影响**：这是 UI 的核心卖点，但没有实现路径。

**方案对比**：

| 方案 | 实现方式 | 延迟 | 带宽 | 可行性 |
|---|---|---|---|---|
| 定时截图 | 每秒 N 帧截图 → 内存传输 | 100-500ms | 中 | 简单，Phase 1 可用 |
| 共享纹理 | 共享 GPU 纹理 | < 16ms | 极低 | 需要工具支持，复杂 |
| 嵌入窗口 | 把工具窗口嵌入 egui | < 16ms | 无 | 平台相关，不稳定 |
| 视频流 | FFmpeg 编码 → WebSocket 推送 | 50-100ms | 高 | 成熟方案 |

**推荐**：Phase 1 用定时截图，Phase 3 探索共享纹理。

#### G11: 多工具输出流的并发显示

**问题**：场景 C 中 Blender 和 Terminal 同时运行，它们的输出如何同时显示在 Command Deck 中？

**影响**：多工具并发时，用户看什么？

**需要**：
- 分屏终端视图（类似 tmux 的 split pane）
- 每个工具的流式输出有独立的颜色标签
- 用户可以切换焦点到特定工具的终端
- 错误输出高亮（红色边框闪烁）

#### G12: 工具进程的优雅关闭

**问题**：`disconnect()` 的定义很简单，但实际场景更复杂：Blender 可能正在渲染、Godot 有未保存的修改。

**影响**：强制关闭可能丢失数据。

**需要**：
```rust
enum ShutdownMode {
    // 立即强制 kill
    Force,
    // 等待当前操作完成，然后关闭
    Graceful { timeout: Duration },
    // 发送关闭信号，不等待
    Signal,
}

async fn shutdown_tool(&self, mode: ShutdownMode) -> Result<ShutdownResult, AdapterError>;
```

---

### 11.3 轻度遗漏（影响完整性和可维护性）

#### G13: 测试策略

**问题**：如何测试多工具编排？单元测试？集成测试？端到端测试？

**需要**：
- **Mock Adapter**：实现 `ToolAdapter` trait，可编程控制行为（延迟、失败、返回特定结果）
- **集成测试**：用 Mock Adapter 测试 Orchestrator 的调度逻辑
- **端到端测试**：需要真实工具安装，CI 中可能难以实现
- **快照测试**：对 Plan 生成结果做快照，防止回归

#### G14: 可观测性 / 调试

**问题**：当 Plan 在第 7 个步骤（共 15 个）失败时，怎么快速定位问题？

**需要**：
- **结构化日志**：每个步骤的 `{plan_id, step_id, tool, command, result, duration, error}`
- **执行追踪**：类似 OpenTelemetry 的 span/trace（每个步骤一个 span）
- **Plan 回放**：保存完整的 Plan + 输入 + 输出，支持事后重放
- **调试面板**：显示当前所有适配器的连接状态、最后 N 次操作

#### G15: 会话持久化

**问题**：WindWave 关闭后重启，正在执行的 Plan 怎么办？已连接的工具有什么状态？

**需要**：
- Plan 执行状态持久化到磁盘
- 重启后可以恢复（询问用户是否继续）
- 工具连接状态不持久化（每次重启重新连接）
- 已完成的步骤结果持久化，避免重复执行

#### G16: 平台兼容性

**问题**：文档假设 macOS。Windows 和 Linux 的 PTY、进程管理、路径都不同。

**需要**：
- 工具路径自动发现：macOS (`/Applications/`), Windows (`Program Files`), Linux (`/usr/bin/`)
- PTY 实现：macOS/Linux 用 `nix::pty`, Windows 用 `ConPTY`
- 路径分隔符处理
- 每个适配器声明平台支持

#### G17: 离线/降级模式

**问题**：没有网络时，LLM 不可用。当前的规则引擎 fallback 能否处理多工具编排？

**影响**：离线时功能降级到接近"不可用"。

**需要**：
- 本地缓存的 Plan 模板（无需 LLM）
- 用户手动创建 Plan（拖拽步骤）
- 明确告知用户哪些功能需要 LLM
- 离线时 UI 显示"离线模式"标识

#### G18: 工具优先级与冲突场景

**问题**：如果 Terminal 和 VS Code 都能编辑文件，Planner 选哪个？如果用户偏好用 VS Code 但 Planner 选了 Terminal？

**需要**：
- 用户可配置工具优先级
- 能力重叠检测（两个工具都声称能做同一件事）
- Orchestrator 在能力重叠时询问用户偏好

#### G19: 许可与合规

**问题**：Figma API 有使用条款限制，Unity 有许可证要求。WindWave 作为桥接器是否需要处理这些？

**需要**：
- 在 Tool Config 中显示工具的许可信息
- 用户首次启用工具时，弹出确认对话框
- 不自动使用需要付费许可证的工具

#### G20: 本地化 / i18n

**问题**：所有 UI 文案硬编码为英文/中文。如果用户是日文/韩文使用者？

**影响**：低优先级，但 Phase 4 需要考虑。

**需要**：Plan 描述、工具能力描述、错误消息的 i18n 框架。

---

### 遗漏总结

```
严重度分布：
  严重 (阻塞实现):  G1-G5    5 个
  中等 (影响质量):  G6-G12   7 个
  轻度 (完整/维护): G13-G20  8 个
  ─────────────────────────────
  总计:             20 个
```

**Phase 1 必须解决**：G1 (Plan 数据结构), G3 (Bevy 异步适配), G4 (取消机制), G5 (工具配置)

**Phase 2 必须解决**：G2 (数据传递), G6 (进度反馈), G7 (Plan 编辑), G8 (跨工具 undo), G9 (能力注入)

**Phase 3 必须解决**：G10 (Live Viewport), G11 (并发输出), G12 (优雅关闭), G13 (测试), G14 (可观测性)

---

## 12. 深度扩散 —— 状态机、边缘案例、协议细节

### 12.1 ToolAdapter 生命周期状态机

每个适配器必须经过严格的状态转换。以下状态机定义了适配器从创建到销毁的完整生命周期：

```
                    ┌─────────┐
                    │  None   │  ← 适配器尚未创建
                    └────┬────┘
                         │ Orchestrator 创建适配器实例
                         ▼
                    ┌─────────┐
          ┌────────→│ Disabled│  ← 用户未启用 / 工具未安装
          │         └────┬────┘
          │              │ 用户启用 + 工具已安装
          │              ▼
          │         ┌─────────┐
          │    ┌───→│ Detected│  ← 发现工具但未连接
          │    │    └────┬────┘
          │    │         │ connect()
          │    │         ▼
          │    │    ┌─────────┐
          │    │    │Connecting│  ← 正在建立连接
          │    │    └────┬────┘
          │    │         │
          │    │    ┌────┴────┐
          │    │    │         │
          │    │    ▼         ▼
          │    │ ┌──────┐ ┌─────────┐
          │    │ │Error │ │ Online  │  ← 连接成功，可执行命令
          │    │ └──┬───┘ └────┬────┘
          │    │    │          │
          │    │    │     ┌────┴────┐
          │    │    │     │         │
          │    │    │     ▼         ▼
          │    │    │ ┌────────┐ ┌──────────┐
          │    │    │ │ Busy   │ │  Idle    │  ← 空闲/忙碌切换
          │    │    │ └───┬────┘ └────┬─────┘
          │    │    │     │          │
          │    │    │     └────┬─────┘
          │    │    │          │
          │    │    │          ▼
          │    │    │     ┌─────────┐
          │    │    │     │Degraded │  ← 部分功能不可用（如某 API 超时）
          │    │    │     └────┬────┘
          │    │    │          │
          │    │    │          ▼
          │    │    │     ┌──────────┐
          │    │    └────→│Disconnected│  ← 主动断开或异常断开
          │    │          └─────┬────┘
          │    │                │
          │    └────────────────┘  (可重连回到 Detected 或 Connecting)
          │
          │         ┌─────────┐
          └─────────│ Removed │  ← 适配器被卸载/删除
                    └─────────┘
```

**状态转换规则**：

| 当前状态 | 触发事件 | 目标状态 | 条件 |
|---|---|---|---|
| `None` | `create_adapter` | `Disabled` | 默认创建 |
| `Disabled` | `enable` | `Detected` | 工具已安装 |
| `Disabled` | `enable` | `Disabled` | 工具未安装 → 提示用户 |
| `Detected` | `connect` | `Connecting` | |
| `Connecting` | `connection_established` | `Online` → `Idle` | |
| `Connecting` | `connection_failed` | `Error` | 可重试 |
| `Connecting` | `timeout` | `Disconnected` | 30s 超时 |
| `Idle` | `execute_start` | `Busy` | |
| `Busy` | `execute_complete` | `Idle` | |
| `Busy` | `execute_error` | `Degraded` | 连续 3 次失败 → Error |
| `Online` | `health_check_fail` | `Degraded` | 心跳丢失 |
| `Degraded` | `health_check_pass` | `Online` → `Idle` | 恢复 |
| `Degraded` | `health_check_fail` × N | `Disconnected` | 连续失败 |
| `Any` | `disconnect` | `Disconnected` | 主动断开 |
| `Disconnected` | `reconnect` | `Connecting` | 自动或手动 |
| `Any` | `remove` | `Removed` | 卸载 |
| `Any` | `crash` | `Disconnected` | 进程崩溃，触发 F1 恢复 |

**状态事件通知**：每次状态转换都通过 `event_stream` 推送 `ToolAdapterEvent::StatusChanged { tool_id, from, to, reason }`，UI 实时更新 Tool Dock。

---

### 12.2 Orchestrator 执行引擎状态机

```
                    ┌──────────┐
                    │   Idle   │  ← 等待用户输入
                    └────┬─────┘
                         │ 用户输入 → Director
                         ▼
                    ┌──────────┐
                    │ Planning │  ← LLM 生成 Plan
                    └────┬─────┘
                         │ Plan 生成完成
                         ▼
                    ┌──────────┐
              ┌────→│PlanReady │  ← 展示 Plan 给用户预览
              │     └────┬─────┘
              │          │ 用户确认 / 自动执行
              │          ▼
              │     ┌──────────┐
              │     │Executing │  ← 正在执行步骤
              │     └────┬─────┘
              │          │
              │     ┌────┴────────────┐
              │     │                 │
              │     ▼                 ▼
              │ ┌─────────┐    ┌──────────┐
              │ │ StepOk  │    │ StepFail │
              │ └────┬────┘    └────┬─────┘
              │      │              │
              │      │         ┌────┴────┐
              │      │         │         │
              │      │         ▼         ▼
              │      │    ┌────────┐ ┌──────────┐
              │      │    │Retrying│ │Rolling   │
              │      │    └───┬────┘ │  Back    │
              │      │        │      └────┬─────┘
              │      │    ┌───┴───┐       │
              │      │    │       │       ▼
              │      │    ▼       ▼   ┌──────────┐
              │      │ ┌──────┐ ┌──┐ │Cancelled │
              │      │ │RetryOk│ │X│ └──────────┘
              │      │ └──┬───┘ └──┘
              │      │    │
              │      └────┼──────┘
              │           │ 还有步骤未执行
              │           ▼
              │      ┌──────────┐
              │      │Next Step │  ← 选择下一个步骤
              │      └────┬─────┘
              │           │ 没有更多步骤
              │           ▼
              │      ┌──────────┐
              │      │Completed │  ← 全部步骤完成
              │      └────┬─────┘
              │           │
              │           ▼
              │      ┌──────────┐
              │      │Reflecting│  ← Reflection 阶段：总结结果
              │      └────┬─────┘
              │           │
              └───────────┘
                         ▼
                    ┌──────────┐
                    │   Idle   │  ← 回到初始状态
                    └──────────┘
```

**关键状态说明**：

| 状态 | 可接收的用户操作 | 说明 |
|---|---|---|
| `Idle` | 输入命令 | 初始状态 |
| `Planning` | 取消 | LLM 推理中，可取消 |
| `PlanReady` | 确认/编辑/拒绝 | 展示 Plan 预览，等待用户决策 |
| `Executing` | 暂停/取消 | 正在执行步骤，可中断 |
| `StepOk` | — | 自动转换到 Next Step |
| `StepFail` | 重试/跳过/回滚 | 步骤失败，等待用户决策 |
| `Retrying` | 取消 | 自动重试中 |
| `RollingBack` | — | 撤销已完成步骤 |
| `Completed` | 查看结果 | 全部完成 |
| `Reflecting` | — | Agent 自我总结 |
| `Cancelled` | 查看日志 | 已被取消 |

**暂停语义**：

```rust
enum PauseAction {
    // 完成当前步骤后暂停
    AfterCurrentStep,
    // 立即暂停（发送 CancellationToken → 步骤尽快终止）
    Immediate,
}
```

暂停后用户可以：
- 继续执行（`Resume`）
- 编辑未执行的步骤（`EditPlan`）
- 取消整个 Plan（`Cancel`）

---

### 12.3 场景边缘案例穷举

基于 4 个场景预演，穷举每个场景的异常路径：

#### 场景 A 边缘案例（创建 3D 角色）

```
正常路径: Blender→Godot→Bevy, 14s, 成功

边缘案例:
  A-E1: Blender 启动超时
    → Orchestrator 重试 3 次 (每次间隔 2s)
    → 全部失败 → 提示用户 "Blender 无法启动，请手动启动后重试"
    → Plan 状态: StepFail → 用户选择 [重试] [跳过Blender步骤] [取消]

  A-E2: Blender 创建 mesh 后崩溃
    → BlenderAdapter 状态: Degraded → Disconnected
    → Orchestrator 检测到 F1 → 自动重启 Blender
    → 重启后查询: "Blender 中是否有 knight mesh？"
    → 有 → 从当前状态继续
    → 无 → 重新执行 Step 1

  A-E3: FBX 导出失败（磁盘满）
    → BlenderAdapter 返回 Error: "No space left on device"
    → Step 3 失败
    → Orchestrator 检查磁盘空间 → 发现 < 100MB
    → 提示用户: "磁盘空间不足，需要至少 50MB。当前可用: 12MB"
    → 等待用户清理 → 重试

  A-E4: Godot 导入 FBX 失败（格式不兼容）
    → GodotAdapter 返回: "FBX version 4.2 not supported by Godot 3.6"
    → Orchestrator 检测到版本冲突 (F7)
    → 方案 A: 降级导出 → Blender 重新导出为 FBX 3.6 格式
    → 方案 B: 提示用户升级 Godot

  A-E5: Bevy 加载预制件失败
    → BevyAdapter 返回: "Asset handle not found: knight.fbx"
    → 检查: 文件是否在正确路径？
    → 是 → 可能是 Bevy asset server 未刷新 → 触发 asset reload
    → 否 → Godot 步骤写了错误路径 → 回滚到 Step 4

  A-E6: 用户中途打开 Blender GUI 手动修改
    → Blender event_stream 推送: "external_modification: knight.blend"
    → Orchestrator 检测到 F9
    → 暂停执行 → 提示用户: "检测到 Blender 中的手动修改，是否覆盖？"
    → [覆盖] [保留手动修改] [合并]
```

#### 场景 B 边缘案例（调试崩溃）

```
正常路径: Terminal→VS Code→Bevy, 5s, 成功

边缘案例:
  B-E1: cargo run 编译失败（语法错误）
    → TerminalAdapter 返回: exit_code=101, "error: expected `;`"
    → 这不是崩溃，是编译错误
    → Orchestrator 需要区分"编译失败"和"运行时崩溃"
    → 编译失败 → 路由到 VS Code 修复语法错误 → 重新编译

  B-E2: 崩溃堆栈被截断
    → TerminalAdapter 输出: "panicked at ... [stack trace truncated]"
    → ResultMerger 无法提取文件位置
    → 降级方案: 设置 RUST_BACKTRACE=full → 重新运行
    → 仍然截断 → 提示用户: "堆栈信息不完整，建议手动运行"

  B-E3: VS Code 未安装
    → Orchestrator 检测 VS Code 不可用
    → 自动降级: 用 TerminalAdapter 的 cat/sed 替代
    → 或: 用 BevyAdapter 内置的代码查看器
    → 提示用户: "VS Code 未安装，已降级为终端编辑"

  B-E4: 修复建议引入了新 bug
    → 用户点击 [Apply Fix] → VSCodeAdapter 修改了文件
    → Reflection 阶段: 建议重新运行测试
    → Orchestrator 自动执行: cargo test → 测试失败
    → 回滚修改 → 重新分析 → 生成备选修复方案

  B-E5: 崩溃不是代码问题（是数据文件问题）
    → 分析发现: 代码正确，但 level_3.dat 文件损坏
    → 修复建议无效（代码不需要改）
    → Reflection 识别: 需要修复数据文件，不是代码
    → 提示用户: "数据文件 level_3.dat 已损坏，建议从备份恢复"
```

#### 场景 C 边缘案例（并发+冲突）

```
正常路径: Blender ∥ Terminal, 8s, 成功

边缘案例:
  C-E1: 两个工具同时完成，顺序不确定
    → Blender 和 Terminal 都在 T+5s 完成
    → 竞态条件: 谁先写入结果？
    → 解决方案: ResultMerger 使用原子 compare-and-swap
    → 两个结果都合并到 PlanContext，不覆盖

  C-E2: 并发工具争抢同一端口
    → Blender MCP 和 Godot MCP 都默认用 8080 端口
    → 第二个工具启动失败: "Address already in use"
    → 解决方案: 动态端口分配
    → Orchestrator 维护 port_registry: HashMap<ToolId, u16>

  C-E3: 并发数量超过系统承载
    → 用户触发 5 个工具同时操作
    → 系统内存不足 → OOM Killer 随机杀进程
    → 解决方案: 并发度限制
    → Orchestrator 配置: max_concurrent_tools = 3
    → 超出限制的步骤排队等待

  C-E4: 一个工具卡死，阻塞整个 Wave
    → Wave 2 有 3 个步骤并发执行
    → Step C 卡死（Blender 死循环）
    → Step D 和 E 已完成，等待 C
    → 解决方案: 单步骤超时不阻塞整个 Wave
    → C 超时后被标记为失败
    → D 和 E 的结果直接进入下一 Wave
    → C 的依赖步骤被跳过
```

#### 场景 D 边缘案例（Figma→Godot UI）

```
正常路径: Figma→Godot, 5s, 成功

边缘案例:
  D-E1: Figma API 限流
    → 首次请求返回 429 Too Many Requests
    → FigmaAdapter 等待 Retry-After 头指定的时间
    → 重试 → 仍然 429 → 提示用户: "Figma API 限流，请稍后重试"
    → 缓存已获取的数据，下次可复用

  D-E2: Figma 设计 Token 无法映射到 Godot
    → Figma 使用了 "Figma-only" 字体
    → Godot 中没有对应的字体资源
    → 转换层: 映射到 Godot 默认字体 + 警告
    → 提示用户: "字体 'Figma Sans' 不可用，已替换为默认字体"

  D-E3: Godot 控件层级嵌套过深
    → Figma 的 Auto Layout 生成了 15 层嵌套
    → Godot 中创建 15 层 Control 节点 → 性能问题
    → 转换层: 优化嵌套 → 合并不必要的层级
    → 提示用户: "Auto Layout 已优化，15 层 → 4 层"

  D-E4: 用户手动修改了 Godot 场景
    → 在 Figma 导出后，用户在 Godot 中手动调整了按钮位置
    → 下次 Figma 同步时，覆盖手动修改
    → 冲突检测: "场景文件自上次同步后被修改"
    → 提示用户: [覆盖] [合并] [跳过]
```

---

### 12.4 MCP 协议消息格式定义

#### 12.4.1 通用信封

```json
// 请求 (WindWave → Tool)
{
  "jsonrpc": "2.0",
  "id": "req_001",
  "method": "tools/call",
  "params": {
    "name": "create_mesh",
    "arguments": {
      "name": "knight",
      "type": "humanoid",
      "position": [0, 0, 0]
    },
    "timeout_ms": 30000
  }
}

// 响应 (Tool → WindWave)
{
  "jsonrpc": "2.0",
  "id": "req_001",
  "result": {
    "status": "success",
    "data": {
      "entity_id": "mesh_knight_001",
      "vertex_count": 1024,
      "triangle_count": 2048
    },
    "execution_time_ms": 350
  }
}

// 错误响应
{
  "jsonrpc": "2.0",
  "id": "req_001",
  "error": {
    "code": -32000,
    "message": "Mesh creation failed",
    "data": {
      "reason": "Name 'knight' already exists in scene",
      "suggestion": "Use a different name or delete existing mesh"
    }
  }
}

// 通知 (Tool → WindWave, 无 id, 不需要响应)
{
  "jsonrpc": "2.0",
  "method": "notifications/progress",
  "params": {
    "step": "sculpt_detail",
    "progress": 0.65,
    "message": "Refining armor plates..."
  }
}
```

#### 12.4.2 各工具的命令定义

**Blender** (Python MCP Server):

```json
// tools/list → 获取所有可用命令
// → 返回: ["create_mesh", "sculpt_detail", "add_armature", "create_animation",
//           "export_fbx", "export_gltf", "set_material", "render_preview",
//           "import_reference", "apply_modifier", "get_scene_info"]

// 创建网格
{
  "method": "tools/call",
  "params": {
    "name": "create_mesh",
    "arguments": {
      "name": "knight",
      "type": "humanoid",    // humanoid | quadruped | custom
      "base_shape": "box",   // box | sphere | cylinder | import
      "segments": { "body": 8, "limbs": 4 },
      "scale": 1.0,
      "position": [0, 0, 0]
    }
  }
}

// 导出 FBX
{
  "method": "tools/call",
  "params": {
    "name": "export_fbx",
    "arguments": {
      "target": "knight",
      "path": "assets/models/knight.fbx",
      "options": {
        "version": "FBX7400",       // 版本兼容性
        "embed_textures": true,
        "apply_modifiers": true,
        "bake_animation": true
      }
    }
  }
}
```

**Godot** (GDScript MCP Server):

```json
// 导入资源
{
  "method": "tools/call",
  "params": {
    "name": "import_asset",
    "arguments": {
      "path": "assets/models/knight.fbx",
      "target_dir": "res://characters/",
      "import_mode": "animated_sprite_3d"
    }
  }
}

// 创建 Control 节点
{
  "method": "tools/call",
  "params": {
    "name": "create_control",
    "arguments": {
      "type": "Button",
      "name": "StartButton",
      "parent": "MainMenu",
      "properties": {
        "text": "Start Game",
        "position": [960, 540],
        "size": [200, 60],
        "theme": "res://themes/cyber_dark.tres"
      }
    }
  }
}
```

**Terminal** (PTY Bridge):

```json
// 执行命令
{
  "method": "tools/call",
  "params": {
    "name": "exec",
    "arguments": {
      "command": "cargo build --release",
      "cwd": "/path/to/project",
      "env": { "RUST_BACKTRACE": "1" },
      "timeout_ms": 300000,
      "capture_output": true
    }
  }
}

// 流式输出（通过 event_stream 推送）
{
  "method": "notifications/output",
  "params": {
    "stream": "stdout",           // stdout | stderr
    "data": "   Compiling windwave v0.3.0\n",
    "timestamp": 1717425600.123
  }
}
```

**VS Code** (Extension MCP Server):

```json
// 打开文件并定位
{
  "method": "tools/call",
  "params": {
    "name": "open_file",
    "arguments": {
      "path": "src/level_loader.rs",
      "line": 142,
      "column": 39,
      "focus": true
    }
  }
}

// 获取代码上下文
{
  "method": "tools/call",
  "params": {
    "name": "get_context",
    "arguments": {
      "path": "src/level_loader.rs",
      "around_line": 142,
      "lines_before": 10,
      "lines_after": 10
    }
  }
}
```

#### 12.4.3 初始化握手

```json
// WindWave → Tool (初始化)
{
  "jsonrpc": "2.0",
  "id": "init_001",
  "method": "initialize",
  "params": {
    "protocol_version": "2024-11-05",
    "client_info": {
      "name": "WindWave",
      "version": "0.3.0"
    },
    "capabilities": {
      "progress_notifications": true,
      "cancellation": true
    }
  }
}

// Tool → WindWave
{
  "jsonrpc": "2.0",
  "id": "init_001",
  "result": {
    "server_info": {
      "name": "Blender MCP Server",
      "version": "1.0.0",
      "tool_version": "Blender 4.2.0"
    },
    "capabilities": {
      "tools": ["create_mesh", "export_fbx", "..."],
      "progress_notifications": true,
      "event_streaming": ["modification", "render_complete", "export_complete"]
    }
  }
}
```

---

### 12.5 配置文件格式

```toml
# ~/.windwave/tools.toml

[tool_registry]
auto_discover = true               # 启动时自动发现已安装的工具
max_concurrent_tools = 3           # 最大并发工具数

[tools.blender]
enabled = true
priority = 1                       # 数字越小越优先
executable_path = "/Applications/Blender.app/Contents/MacOS/Blender"
mcp_server_path = "~/.windwave/mcp-servers/blender_mcp_server.py"
auto_start = true                  # Plan 需要时自动启动
startup_timeout_secs = 30          # 启动超时
python_path = "/Applications/Blender.app/Contents/MacOS/Blender"  # 或 python3
background_mode = true             # 使用 --background 模式
custom_args = ["--factory-startup"] # 额外启动参数
environment = { "BLENDER_USER_SCRIPTS" = "~/.windwave/blender_scripts" }

[tools.godot]
enabled = true
priority = 2
executable_path = "/Applications/Godot.app/Contents/MacOS/Godot"
mcp_server_path = "~/.windwave/mcp-servers/godot_mcp_server.gd"
auto_start = true
startup_timeout_secs = 20
project_path = "~/"                # 默认 Godot 项目路径
# 或是特定项目
# project_path = "/path/to/game/project"

[tools.terminal]
enabled = true
priority = 10
shell = "zsh"                      # zsh | bash | fish | powershell
allowed_commands = ["cargo", "git", "pnpm", "npm", "node", "rustc", "rustfmt", "clippy"]
working_directory = "~/"           # 默认工作目录
timeout_default_secs = 300         # 默认命令超时

[tools.vscode]
enabled = false                    # 默认禁用（需要安装扩展）
priority = 5
executable_path = "/Applications/Visual Studio Code.app/Contents/MacOS/Electron"
extension_id = "windwave.vscode-agent"  # 需要安装的扩展
auto_start = true

[tools.figma]
enabled = false
priority = 20
# API Key 从环境变量或 macOS Keychain 读取
api_key_source = "keychain"        # keychain | env | config
# api_key = "figd_..."             # 不推荐直接写在配置文件中
rate_limit = { requests_per_minute = 60 }
cache_enabled = true
cache_ttl_secs = 300               # 缓存 5 分钟

[tools.git]
enabled = true
priority = 8
# 使用 libgit2 直连，不需要外部进程
author_name = "WindWave Agent"
author_email = "agent@windwave.local"
```

---

### 12.6 错误分类体系

```rust
/// 完整的错误类型层次结构
/// 每个错误都携带: 错误码、人类可读消息、可重试标志、建议操作

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AdapterError {
    // ── 连接层错误 ──
    Connection(ConnectionError),
    // ── 协议层错误 ──
    Protocol(ProtocolError),
    // ── 执行层错误 ──
    Execution(ExecutionError),
    // ── 资源层错误 ──
    Resource(ResourceError),
    // ── 安全层错误 ──
    Security(SecurityError),
    // ── 工具层错误 ──
    Tool(ToolError),
    // ── 编排层错误 ──
    Orchestration(OrchestrationError),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConnectionError {
    Timeout { after_secs: u32 },
    Refused { reason: String },
    Broken { reason: String },
    AuthenticationFailed { reason: String },
    HandshakeFailed { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProtocolError {
    InvalidRequest { detail: String },
    InvalidResponse { detail: String },
    UnsupportedVersion { expected: String, got: String },
    MethodNotFound { method: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionError {
    CommandFailed { command: String, exit_code: i32, stderr: String },
    Timeout { command: String, after_ms: u64 },
    Cancelled { by_user: bool },
    InvalidArguments { detail: String },
    ToolNotFound { tool_id: String },
    ToolNotReady { tool_id: String, status: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResourceError {
    FileNotFound { path: String },
    DiskFull { available_bytes: u64, needed_bytes: u64 },
    PermissionDenied { path: String },
    AssetConflict { path: String, modified_by: String },
    OutOfMemory { used_bytes: u64, limit_bytes: u64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SecurityError {
    CommandNotAllowed { command: String },
    ForbiddenCharacters { command: String, chars: String },
    ApiKeyMissing { tool: String },
    RateLimited { tool: String, retry_after_secs: u32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ToolError {
    Crashed { signal: Option<i32>, exit_code: Option<i32> },
    Hung { last_activity_secs: u64 },
    VersionMismatch { expected: String, got: String },
    InternalError { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OrchestrationError {
    PlanInvalid { reason: String },
    PlanCircular { steps: Vec<String> },
    StepDependencyMissing { step_id: String, dep_id: String },
    NoToolAvailable { capability: String },
    DegradedToolRejected { tool_id: String, reason: String },
}

// 每个错误都携带恢复建议
impl AdapterError {
    pub fn is_retryable(&self) -> bool {
        match self {
            AdapterError::Connection(_) => true,
            AdapterError::Execution(ExecutionError::Timeout { .. }) => true,
            AdapterError::Tool(ToolError::Crashed { .. }) => true,
            AdapterError::Resource(ResourceError::DiskFull { .. }) => false,
            AdapterError::Security(_) => false,
            _ => false,
        }
    }

    pub fn user_suggestion(&self) -> String {
        match self {
            AdapterError::Connection(ConnectionError::Timeout { after_secs }) =>
                format!("连接超时（{}s），请检查工具是否正在运行", after_secs),
            AdapterError::Resource(ResourceError::DiskFull { available_bytes, needed_bytes }) =>
                format!("磁盘空间不足，需要 {}MB，可用 {}MB，请清理磁盘后重试",
                    needed_bytes / 1_000_000, available_bytes / 1_000_000),
            AdapterError::Security(SecurityError::CommandNotAllowed { command }) =>
                format!("命令 '{}' 不在允许列表中，如需使用请在设置中添加", command),
            // ...
            _ => "未知错误".to_string(),
        }
    }
}
```

---

### 12.7 压力测试场景

**场景 S1: 大规模 Plan（100 个步骤）**

```
输入: "全面重构整个游戏项目"
Plan: 100 个步骤，涉及 6 个工具
预期: 
  - 内存占用 < 500MB (Plan 本身)
  - UI 渲染 < 16ms (Agent Pipeline 100 个节点)
  - 调度延迟 < 1ms (拓扑排序 100 个节点)
风险: 
  - 100 个步骤的依赖图可能包含循环
  - 用户无法在 UI 中浏览 100 个步骤
缓解: 
  - 步骤分组（将 100 步折叠为 10 个组）
  - 分页显示
  - 循环检测在 Plan 验证阶段执行
```

**场景 S2: 10 个工具同时运行**

```
输入: 10 个独立工具的并发操作
预期: 
  - 系统内存: 10 × 500MB = 5GB (可能超出)
  - CPU: 10 个进程竞争
  - IPC: 10 个 stdio/TCP 连接
风险:
  - 内存不足导致 OOM
  - 端口耗尽
  - 文件描述符耗尽
缓解:
  - max_concurrent_tools = 3 (限制并发)
  - 超出限制的工具排队
  - 内存监控: 超过 80% 时暂停新工具启动
```

**场景 S3: 单个工具持续运行 30 分钟**

```
输入: Terminal: "cargo build --release" (大型项目)
预期: 编译 30 分钟
风险:
  - 用户以为卡死
  - 心跳超时（如果心跳间隔 < 30min）
  - 输出缓冲区溢出
缓解:
  - 进度通知: Terminal 每 5 秒推送一次编译进度
  - 心跳间隔: 调整为 60s
  - 输出缓冲区: 环形缓冲区，只保留最近 10000 行
```

**场景 S4: 快速连续触发 5 个 Plan**

```
输入: 用户连续发出 5 个命令
预期: 5 个 Plan 排队执行
风险:
  - Plan 之间可能互相冲突
  - 用户意图不明确
缓解:
  - 队列模式: 给用户展示 "Plan Queue" 面板
  - 用户可以: [调整顺序] [取消] [合并 Plan]
  - 新 Plan 如果修改了正在执行的 Plan 的资源 → 提示冲突
```

---

### 12.8 扩散总结

本次扩散新增内容覆盖了 5 个关键维度：

| 维度 | 内容 | 颗粒度 |
|---|---|---|
| **状态机** | ToolAdapter 14 状态 + Orchestrator 11 状态 | 完整状态转换表 |
| **边缘案例** | 4 个场景 × 4-6 个异常路径 = 19 个 | 逐一穷举 + 恢复方案 |
| **协议细节** | MCP 信封 + 5 个工具的具体命令 + 初始化握手 | JSON 格式定义 |
| **配置系统** | 7 个工具的 TOML 配置 | 完整 schema |
| **错误体系** | 7 大类 × 37 种子错误 + 恢复建议 | Rust 枚举定义 |

**后续还需要扩散的领域**（留给下一轮，按优先级排列）：
- UI 面板的 pixel-perfect 线框图
- 每个工具适配器的单元测试用例
- 性能基准测试的具体指标
- CI/CD 流水线设计
- 用户文档结构

---

## 13. GitHub 生态深度探索 —— 可借鉴的开源项目 (>10K Stars)

> 探索 GitHub 上与 WindWave 4 个核心领域相关的顶级开源项目，分析架构、吸取经验、识别合作机会。

### 13.1 领域一：AI Agent 框架（多 Agent 编排）

#### LangChain (langchain-ai/langchain) — 120K+ Stars, Python

**核心理念**：模块化 LLM 应用编排，通过 Chain/Agent/Memory 抽象快速构建 RAG、对话助手。

**对 WindWave 的借鉴价值**：
- **Tool 抽象**：`BaseTool` 的设计（name, description, args_schema, _run）几乎直接对应 WindWave 的 `ToolAdapter` trait。可以借鉴其 tool description 生成方式，自动注入到 Planner 的 system prompt 中
- **Memory 系统**：ConversationBufferMemory、VectorStoreRetryMemory 等分层记忆，可参考其实现来增强 WindWave 的跨会话 Agent 记忆
- **LangSmith 可观测性**：全链路 trace 追踪 + token 消耗统计，直接对标 WindWave 的 G14（可观测性需求）

**注意**：LangChain 的 Tool 是同步 LLM 调用模式，WindWave 需要的是异步 MCP 进程间通信，不能直接复用，但抽象层设计可参考。

#### LangGraph (langchain-ai/langgraph) — 19K+ Stars, Python

**核心理念**：基于有向图的状态机编排，支持条件分支、循环、人机协作。

**对 WindWave 的借鉴价值**：
- **Graph-based 编排**：LangGraph 的 StateGraph 概念直接对标 WindWave 的 Orchestrator 执行引擎。其节点/边/条件边的设计可以借鉴来实现 Plan 的拓扑排序和条件路由
- **Checkpointer 持久化**：支持将图状态持久化到数据库，从断点恢复执行。直接对标 WindWave 的 G15（会话持久化）
- **Human-in-the-Loop**：`interrupt()` 机制在任意节点暂停等待人工审批，对标 WindWave 的 `approval_required` 字段

**关键差异**：LangGraph 是单一 LLM 进程内的编排，WindWave 是跨多个外部进程的编排。但状态机设计模式完全可复用。

#### CrewAI (joaomdmoura/crewai) — 39K+ Stars, Python

**核心理念**：角色扮演式多 Agent 协作，每个 Agent 有 role/goal/backstory/tools。

**对 WindWave 的借鉴价值**：
- **角色分工设计**：CrewAI 的 "Agent 角色" 概念可以映射到 WindWave 的 "Agent 角色色"（Director=品红, Artist=青蓝, Engineer=电紫, QA=翠绿, System=琥珀）
- **Task 依赖链**：Task 的 `context` 字段指定前置任务，直接对标 WindWave 的 PlanStep.dependencies
- **Process 类型**：sequential（串行）和 hierarchical（层级委派），对标 WindWave 的 ExecutionMode

**局限性**：CrewAI 是纯 Python 且假设所有 Agent 在同一进程中运行 LLM 推理，不适合 WindWave 的跨工具场景。

#### AutoGen (microsoft/autogen) — 50K+ Stars, Python

**核心理念**：异步多 Agent 对话，事件驱动协调。

**对 WindWave 的借鉴价值**：
- **异步事件驱动**：Agent 之间通过消息传递协调，而非同步调用。这更接近 WindWave 的跨工具异步通信模型
- **GroupChat 模式**：多个 Agent 在同一个 "对话" 中协作，可参考其消息路由机制
- **Code Executor**：支持在 Docker 沙箱中执行代码，对标 WindWave 的 TerminalAdapter 安全模型

#### OpenAI Agents SDK (openai/openai-agents-python) — 20K+ Stars, Python

**核心理念**：轻量级 Agent 抽象，guardrails（护栏）+ handoffs（移交）+ tracing（追踪）。

**对 WindWave 的借鉴价值**：
- **Handoffs 模式**：Agent 可以将任务 "移交" 给另一个 Agent，直接对标 WindWave 的 Orchestrator 工具路由
- **Guardrails**：输入/输出验证，对标 WindWave 的安全检查（命令白名单、输入过滤）
- **Tracing**：内置 OpenTelemetry 风格的追踪，对标 G14

---

### 13.2 领域二：MCP 生态（工具桥接协议）

#### awesome-mcp-servers (punkpeye/awesome-mcp-servers) — 73K+ Stars

**核心理念**：MCP Server 的精选目录，涵盖 2000+ 个 MCP Server 实现。

**对 WindWave 的借鉴价值**：
- **工具发现**：这份列表展示了社区已经为哪些工具构建了 MCP Server。WindWave 的 ToolAdapter 可以优先实现社区已有 MCP Server 的工具（减少适配器开发量）
- **分类体系**：Database、File System、Version Control、Cloud 等分类，可参考来设计 WindWave 的 Tool Dock 分组
- **生态整合**：WindWave 的 ToolAdapter 可以直接复用社区现有的 MCP Server（如 GitHub MCP Server、Playwright MCP），而不是重新实现

#### GitHub MCP Server (github/github-mcp-server) — 24K+ Stars, Go

**核心理念**：GitHub 官方 MCP Server，100+ 工具覆盖仓库管理、Issue、PR、Actions 等。

**对 WindWave 的借鉴价值**：
- **生产级 MCP 实现案例**：每周 700 万次 tool call，95% 成功率。它的架构挑战（上下文窗口膨胀、工具过滤、OAuth 安全）直接对应 WindWave 的 G4/G5/G9 问题
- **动态工具过滤**：基于用户权限动态过滤可用工具，避免上下文窗口膨胀。WindWave 的 Orchestrator 也需要类似机制
- **Stateless + Redis 会话**：无状态架构 + Redis 存储会话，可参考此模式实现 WindWave 的会话持久化
- **评估框架**：GitHub 团队构建了专门的 tool call 评估框架，WindWave 的测试策略（G13）可以借鉴

#### Playwright MCP (microsoft/playwright-mcp) — 22K+ Stars, TypeScript

**核心理念**：浏览器自动化 MCP Server，让 AI 能直接操控浏览器。

**对 WindWave 的借鉴价值**：
- **浏览器操控能力**：如果 WindWave 需要操作 Web 工具（如 Figma Web、在线编辑器），可以直接复用 Playwright MCP
- **截图/视觉验证**：对标 WindWave 的 Live Viewport（G10），可以用 Playwright 截图作为 Web 工具的预览方案
- **结构化可访问性快照**：Playwright 的 accessibility snapshot 比像素截图更适合 AI 理解，可参考做工具预览

#### Context7 (upstash/context7) — 35K+ Stars, JavaScript

**核心理念**：为 LLM 和 AI 代码编辑器提供实时代码文档的 MCP Server。

**对 WindWave 的借鉴价值**：
- **实时文档注入**：自动生成项目的结构文档并注入到 LLM context 中。WindWave 的 Planner 需要类似的能力——将工具能力（capabilities）注入到 LLM prompt 中（G9）
- **增量更新**：代码变更时自动更新文档，对标 WindWave 的知识图谱增量更新需求

#### CodePrism (dragonscale-ai/codeprism) — ~2K Stars, Rust

**核心理念**：Rust 实现的代码知识图谱 MCP Server，支持 5 种语言，< 50ms 查询延迟。

**对 WindWave 的借鉴价值**：
- **Rust 原生实现**：与 WindWave 技术栈一致（Rust + Bevy），可以直接集成或参考其代码
- **MCP 原生**：First-class MCP 实现，可参考其 MCP Server 的 Rust 实现模式
- **Tree-sitter 解析**：使用 Tree-sitter 做 AST 解析，WindWave 也使用 web-tree-sitter（见 CLAUDE.md），可参考其解析管道设计
- **MIT 许可证**：可以自由使用和修改

---

### 13.3 领域三：游戏引擎 AI 工具

#### Unity MCP Server (CoplayDev/unity-mcp) — 9.7K Stars, C#/Python

**核心理念**：通过 MCP 协议让 AI 直接操控 Unity Editor，36+ 工具覆盖场景、资源、脚本、物理、渲染等。

**对 WindWave 的借鉴价值**：
- **最接近 WindWave 目标的架构**：Unity MCP = AI → MCP Server → Unity Editor。WindWave = AI → ToolAdapter → 外部工具。架构几乎一致
- **工具分类设计**：Scene/Object、Asset/Material、Scripting、Physics、Profiling 等分类，可直接参考来设计 WindWave 的 ToolCommand 枚举
- **batch_execute 优化**：批量执行多个操作（10-100x 速度提升），WindWave 的 Orchestrator 也需要批处理优化
- **Bridge + Relay 双组件**：Unity 包内运行 Bridge + 外部 Relay 进程。这种 "进程内桥接 + 进程外中继" 模式直接适用于 WindWave 的 Blender/Godot 适配器
- **编辑器状态查询**：`read_console`、`debug_context` 等诊断工具，对标 WindWave 的 G14（可观测性）

**关键局限**：目前 9.7K stars，未达到 10K 门槛，但增长速度极快（几周内从 0 到 9.7K），很快会突破。

#### PlayCanvas Editor MCP Server (playcanvas/editor-mcp-server) — 官方项目

**核心理念**：通过 MCP 让 AI 直接操控 PlayCanvas Web 编辑器（WebGL 游戏引擎）。

**对 WindWave 的借鉴价值**：
- **Web 引擎的 MCP 集成**：PlayCanvas 是 Web 端引擎，其 MCP 集成通过 Chrome Extension + WebSocket 实现。这个模式可以推广到任何 Web 工具
- **实时反馈**：WebSocket 实时推送编辑器状态变更，对标 WindWave 的 `event_stream()` 设计
- **自然语言驱动**：用户说 "create a sphere" → AI 直接操作编辑器，这就是 WindWave 的 Command Deck 核心理念

#### Blender MCP 社区项目

目前 Blender 还没有官方的 MCP Server，但社区有多个实现：
- blender-mcp (ahujasid/blender-mcp) — 社区项目，约 1.5K stars
- blender-mcp-tools — 多个小型实现
- **对 WindWave 的启示**：这意味着 WindWave 的 BlenderAdapter 需要自己实现 MCP Server，这是 Phase 2 的核心工作

#### Godot MCP 社区项目

- godot-mcp (社区项目) — 约 500 stars
- **对 WindWave 的启示**：同样缺乏成熟的官方实现，WindWave 需要自己构建

---

### 13.4 领域四：代码智能与知识图谱

#### GitNexus (abhigyanpatwari/GitNexus) — 39K+ Stars, TypeScript

**核心理念**：代码知识图谱引擎，12 阶段 DAG 管道 + 图数据库 + MCP 原生。

**对 WindWave 的借鉴价值**：
- **12 阶段解析管道**：类型解析 → 调用链 → 入口点评分 → 社区检测 → 向量嵌入。WindWave 的代码理解模块可以参考这个管道设计
- **Leiden 社区检测**：将代码自动聚类为功能模块，这个算法可以用于 WindWave 的 Scene Index 自动分组
- **双模式运行**：CLI 本地 + 浏览器端 WASM 双模式。WindWave 也可以考虑将知识图谱在浏览器端渲染（对标理解工具的 Web Dashboard）
- **Process（执行流）**：将跨文件调用链抽象为业务执行流程，这是 WindWave 的 Scene Index 可以借鉴的高层次抽象
- **LadybugDB**：嵌入式图数据库，带原生向量支持。可评估是否替代 WindWave 当前的存储方案

**许可证问题**：PolyForm Noncommercial — 商业使用受限。WindWave 不能直接复用，但可以学习其架构思路。

#### Repomix (yamadashy/repomix) — 22K+ Stars, TypeScript

**核心理念**：将整个代码仓库打包为一个 AI 友好的文本文件（上下文打包）。

**对 WindWave 的借鉴价值**：
- **上下文打包**：将代码库结构化为 LLM 友好的格式。WindWave 的 Planner 需要将代码上下文注入到 LLM prompt 中，Repomix 的输出格式可以参考
- **Token 计数与优化**：自动计算 token 消耗，优化输出大小。直接对标 WindWave 的上下文管理需求
- **轻量级方案**：对于不需要完整知识图谱的场景，Repomix 的方案更轻量。WindWave 可以同时支持两种模式

#### code2prompt (mufeedvh/code2prompt) — 7.2K Stars, Rust

**核心理念**：Rust 实现的代码库到 LLM prompt 转换器，生成带行号的代码树。

**对 WindWave 的借鉴价值**：
- **Rust 实现**：与 WindWave 技术栈一致
- **模板系统**：支持自定义 Handlebars 模板，可以定制输出格式。WindWave 的 Planner prompt 生成可以借鉴这个模板系统

#### Sourcegraph Cody (sourcegraph/cody) — 大平台

**核心理念**：基于代码知识图谱的 AI 编码助手，支持代码搜索、解释、重构。

**对 WindWave 的借鉴价值**：
- **企业级代码智能**：为大型代码库提供语义搜索和上下文理解，对标 WindWave 的 Scene Index
- **MCP 集成**：2025 年新增 MCP 支持，可作为 WindWave 的 VS Code 适配器参考实现

---

### 13.5 领域五：AI 代码生成与编辑

#### Aider (paul-gauthier/aider) — 25K+ Stars, Python

**核心理念**：终端 AI 结对编程工具，支持多文件编辑、Git 集成、repo-map 上下文。

**对 WindWave 的借鉴价值**：
- **Repo-map 上下文**：自动生成代码库的结构化地图注入 LLM context。这是 WindWave 的 Planner 需要的核心能力
- **多文件编辑**：一次对话中修改多个文件，自动生成 Git commit。对标 WindWave 的跨工具编辑能力
- **编辑格式**：使用 search/replace 块格式让 LLM 生成精确编辑，可参考其 prompt 设计

#### OpenHands (All-Hands-AI/OpenHands) — 45K+ Stars, Python

**核心理念**：AI 驱动的软件开发 Agent，能自主编写代码、运行命令、浏览网页。

**对 WindWave 的借鉴价值**：
- **自主开发循环**：读代码 → 理解 → 修改 → 运行测试 → 迭代。这个循环直接对标 WindWave 的 Director 调试场景（场景 B）
- **Sandbox 执行**：Docker 沙箱中执行代码，对标 WindWave 的 TerminalAdapter 安全模型
- **Web 浏览**：内置浏览器交互能力，对标 WindWave 的 Figma 适配器

#### Continue (continuedev/continue) — 20K+ Stars, TypeScript

**核心理念**：开源 AI 代码助手，IDE 插件（VS Code / JetBrains）。

**对 WindWave 的借鉴价值**：
- **IDE 集成模式**：如何作为插件嵌入 VS Code 并提供 AI 能力。WindWave 的 VS Code 适配器可以参考这个模式
- **Slash Commands**：自然语言命令系统，对标 WindWave 的 Command Deck
- **MCP 支持**：支持通过 MCP 接入外部工具，与 WindWave 的 MCP 战略一致

---

### 13.6 领域六：Rust 游戏引擎与编辑器

#### Bevy (bevyengine/bevy) — 35K+ Stars, Rust

**核心理念**：Rust 生态最流行的 ECS 游戏引擎，WindWave 的底层引擎。

**对 WindWave 的借鉴价值**：
- WindWave 已在 Bevy 之上构建，但可以持续关注 Bevy 的 Editor 进展（bevy_editor_prototypes）
- Bevy 0.16+ 的 UI 系统改进可能影响 WindWave 的 UI 层
- Bevy 的 Asset System 重写（RFC 57）可能影响 WindWave 的资源管理

#### Fyrox (FyroxEngine/Fyrox) — 8K+ Stars, Rust

**核心理念**：Rust 原生游戏引擎，自带场景编辑器。

**对 WindWave 的借鉴价值**：
- **自带编辑器**：Fyrox 的编辑器是用引擎自身构建的（dogfooding），WindWave 也是同样的理念
- **场景编辑器 UX**：可参考其编辑器的交互设计（Inspector、Hierarchy、Scene View）

---

### 13.7 综合评价与优先级

| 排名 | 项目 | Stars | 与 WindWave 的关联度 | 核心借鉴点 | 借鉴优先级 |
|---|---|---|---|---|---|
| 1 | Unity MCP | 9.7K | **极高** — 架构几乎一致 | Bridge+Relay 双组件、36 工具分类、batch_execute | **P0** |
| 2 | LangGraph | 19K | **极高** — 状态机编排 | StateGraph、Checkpointer、HITL interrupt | **P0** |
| 3 | GitHub MCP | 24K | **高** — 生产级 MCP 案例 | 工具过滤、OAuth、评估框架、700万/周调用 | **P0** |
| 4 | OpenHands | 45K | **高** — 自主开发循环 | Sandbox 执行、读→改→测循环 | **P1** |
| 5 | GitNexus | 39K | **高** — 知识图谱 | 12 阶段管道、Leiden 聚类、Process 执行流 | **P1** |
| 6 | CrewAI | 39K | **中** — 角色分工 | Agent 角色色设计、Task 依赖链 | **P1** |
| 7 | CodePrism | 2K | **高** — Rust+MCP | MCP Server Rust 实现、Tree-sitter 管道 | **P1** |
| 8 | Aider | 25K | **中** — 代码编辑 | Repo-map 上下文、多文件编辑 | **P2** |
| 9 | Repomix | 22K | **中** — 上下文打包 | 代码库到 LLM 格式转换、Token 优化 | **P2** |
| 10 | Playwright MCP | 22K | **中** — 浏览器操控 | 浏览器自动化、截图/视觉验证 | **P2** |
| 11 | awesome-mcp | 73K | **中** — 生态目录 | 工具发现、分类体系 | **P2** |
| 12 | PlayCanvas MCP | 官方 | **中** — 游戏引擎 MCP | WebSocket 实时反馈、自然语言驱动 | **P3** |

**启示总结**：

1. **Unity MCP 是我们最应该研究的项目** — 它的架构（Bridge+Relay）、工具分类、批量执行优化、编辑器状态查询，几乎直接对应 WindWave 的 ToolAdapter 设计。建议在 Phase 1 就深入阅读其源码。

2. **LangGraph 的 StateGraph + Checkpointer 是 Orchestrator 的最佳参考** — 状态机设计、持久化、人机协作中断，这三个概念直接解决 WindWave 的 G4/G7/G15 问题。

3. **GitHub MCP 的工程实践值得学习** — 100+ 工具导致的上下文膨胀、动态工具过滤、OAuth 安全、评估框架，这些是 WindWave 在 Phase 3 多工具场景下必然遇到的问题。

4. **MCP 生态已经成熟，应该优先复用而非重新实现** — playwright-mcp、github-mcp-server 等可以直接作为 WindWave 的 ToolAdapter 底层实现，大大减少开发量。

5. **Blender 和 Godot 的 MCP Server 是空白领域** — 社区没有成熟的实现，WindWave 需要自己构建。这是 Phase 2 的核心工作，也是建立差异化竞争力的机会。

---

## 14. 研究深度分析 —— 架构解构与可复用模式

> 对 P0 优先级的 4 个核心项目进行架构解构，提取可直接应用于 WindWave 的代码模式和设计决策。

### 14.1 Unity MCP 深度解构

**项目**：[CoplayDev/unity-mcp](https://github.com/CoplayDev/unity-mcp) | 9.7K stars | MIT | 1,475 commits | C# 70% + Python 28%

**推荐阅读**：[CLAUDE.md](https://github.com/CoplayDev/unity-mcp/blob/beta/CLAUDE.md) — 极其清晰的架构文档，值得 WindWave 效仿。

#### 14.1.1 三层架构（Python 端）

```
AI Assistants (Claude/Cursor/Windsurf)
    ↓ MCP Protocol (stdio 或 HTTP)
┌─────────────────────────────────────────┐
│  Python Server (Server/src/)            │
│  ┌─────────┐ ┌──────────┐ ┌──────────┐ │
│  │MCP Tools│ │CLI Cmds  │ │Resources │ │
│  │@mcp_for │ │@click    │ │@mcp_for  │ │
│  │_unity   │ │.command  │ │_unity     │ │
│  │_tool    │ │          │ │_resource │ │
│  └────┬────┘ └────┬─────┘ └────┬─────┘ │
│       │           │            │       │
│       │     send_with_unity_instance   │
│       │     (WebSocket + HTTP)         │
└───────┼───────────┼────────────┼───────┘
        │           │            │
        ↓           ↓            ↓
┌─────────────────────────────────────────┐
│  Unity Editor Plugin (MCPForUnity/)     │
│  ┌──────────────────────────────────┐   │
│  │   CommandRegistry (反射注册)      │   │
│  │   [McpForUnityTool("manage_*")]   │   │
│  │   HandleCommand(params)           │   │
│  └──────────────────────────────────┘   │
│              ↓                          │
│        Unity Editor API                 │
└─────────────────────────────────────────┘
```

**三种传输模式**：

| 模式 | 适用场景 | 通信方式 | 备注 |
|---|---|---|---|
| **Stdio** | 单 Agent | Python 进程 → 旧 TCP bridge → Unity | 新连接覆盖旧连接 |
| **HTTP** | 多 Agent | 共享 Python Server → WebSocket hub → Unity | session 隔离 via client_id |

#### 14.1.2 工具注册模式（Python 端）

这是 Unity MCP 最精妙的设计——**自动发现 + 分组机制**：

```python
@mcp_for_unity_tool(
    description="Manage scenes in the Unity Editor.",
    group="core",  # core(默认启用) | vfx | animation | ui | scripting_ext | testing 等
)
async def manage_scene(
    ctx: Context,
    action: Annotated[Literal["create", "load", "save", "switch"], "Action to perform"],
) -> dict[str, Any]:
    unity_instance = await get_unity_instance_from_context(ctx)
    response = await send_with_unity_instance(
        async_send_command_with_retry, unity_instance, "manage_scene", {"action": action}
    )
    return response
```

**分组机制**：只有 `group="core"` 的工具默认启用。非 core 工具（vfx, animation, probuilder 等）通过 `manage_tools` 动态开启/关闭。这直接对应 WindWave 的 **动态工具过滤** 需求（G9）。

#### 14.1.3 工具注册模式（C# 端）

```csharp
[McpForUnityTool("manage_something", AutoRegister = false, Group = "core")]
public static class ManageSomething
{
    // 同步处理器（大多数工具）
    public static object HandleCommand(JObject @params)
    {
        var p = new ToolParams(@params);
        var name = p.RequireString("name");
        // ... 操作 ...
        return new SuccessResponse("Done.", new { data = result });
    }
    
    // 异步处理器（长操作：play-test、refresh、batch）
    public static async Task<object> HandleCommand(JObject @params)
    {
        // CommandRegistry 自动检测 Task 返回类型
        await SomeAsyncOperation();
        return new SuccessResponse("Done.");
    }
}
```

**参数验证模式**：
```csharp
var p = new ToolParams(parameters);
var pageSize = p.GetInt("page_size", "pageSize") ?? 50;  // 可选参数 + 别名
var name = p.RequireString("name");                       // 必填参数
```

#### 14.1.4 对 WindWave 的直接借鉴

| Unity MCP 概念 | WindWave 对应 | 借鉴方式 |
|---|---|---|
| `@mcp_for_unity_tool(group="core")` | `ToolAdapter` 的 `capabilities()` | 用 trait 方法返回 capabilities + 分组 |
| `CommandRegistry` 反射注册 | `ToolOrchestrator` 的 plan 路由 | 用 trait 的 `execute()` 分发 |
| `ToolParams` (GetInt/RequireString) | `ToolCommand` 参数解析 | 定义带验证的参数结构体 |
| `send_with_unity_instance` | `adapter.execute()` | 同上，已设计 |
| 分组机制（core/vfx/animation/...) | Tool Dock 分组显示 | 可将工具按类型分组到 Tool Dock |
| `batch_execute` | 批量命令执行 | 在 Phase 1 加入 batch_execute |
| `read_console` / `debug_context` | G14 可观测性 | 实现类似诊断工具 |

**Unity MCP 的教训**（来自 CLAUDE.md 的 "Code Philosophy"）：
1. **Domain Symmetry** — Python 工具 ↔ C# 工具一一对应。WindWave 的 `ToolAdapter::execute()` 同样需要对称映射到目标工具的命令
2. **Minimal Abstraction** — 避免过早抽象。这在 WindWave 的 Phase 1 尤其重要
3. **Keep Tools Focused** — 每个工具做一件事。WindWave 的 ToolCommand 应该拆分为小而专注的命令

---

### 14.2 LangGraph 深度解构

**项目**：[langchain-ai/langgraph](https://github.com/langchain-ai/langgraph) | 19K+ stars | MIT | 6,935 commits | Python + TypeScript

#### 14.2.1 StateGraph 核心模式

LangGraph 的本质是一个 **有向状态机**，节点是处理函数，边是状态转换规则：

```python
from langgraph.graph import StateGraph, START, END

class WindwavePlanState(TypedDict):
    plan_id: str
    steps: list[PlanStep]
    current_step_index: int
    step_results: dict[str, ToolResult]
    errors: list[str]

builder = StateGraph(WindwavePlanState)

# 每个节点是 WindWave 的一个执行阶段
builder.add_node("planning", planning_node)        # LLM 生成 Plan
builder.add_node("execute_step", execute_step_node) # Orchestrator 分发
builder.add_node("resolve_conflict", resolve_node)  # 冲突解决
builder.add_node("reflect", reflect_node)           # 事后反思

# 条件边 = Orchestrator 的路由逻辑
builder.add_conditional_edges(
    "execute_step",
    route_next,  # 返回 "continue" | "resolve_conflict" | "reflect" | "end"
    {"continue": "execute_step", "resolve_conflict": "resolve_conflict", "reflect": "reflect"}
)

builder.add_edge(START, "planning")
builder.add_edge("planning", "execute_step")
builder.add_edge("resolve_conflict", "execute_step")  # 解决冲突后继续执行
builder.add_edge("reflect", END)

graph = builder.compile(checkpointer=checkpointer)
```

**条件路由函数** — 这是 Orchestrator 的核心：
```python
def route_next(state: WindwavePlanState) -> str:
    """根据当前状态决定下一步"""
    if state["errors"]:
        return "resolve_conflict"
    if state["current_step_index"] >= len(state["steps"]):
        return "reflect"
    return "continue"
```

#### 14.2.2 持久执行（Durable Execution）

这是 LangGraph 最重要的特性——**崩溃恢复**：

```python
# 三种持久化模式
graph.invoke(input, config, durability="exit")   # 最快，完成后才保存
graph.invoke(input, config, durability="async")  # 异步保存，推荐
graph.invoke(input, config, durability="sync")   # 同步保存，最安全

# thread_id 是持久化游标
config = {"configurable": {"thread_id": "plan-001"}}
graph.invoke({"goal": "创建3D角色"}, config)
# 即使进程崩溃，restart 后：
graph.invoke(None, config)  # 自动从上次中断处恢复
```

**WindWave 对应设计**：
```rust
enum DurabilityMode {
    Exit,     // Plan 完成后才持久化到磁盘
    Async,    // 异步写入（推荐，适合大多数场景）
    Sync,     // 同步写入（关键操作）
}

// thread_id 映射到 WindWave 的 PlanId
struct PlanContext {
    plan_id: PlanId,
    checkpoint: Option<Checkpoint>,
    durability: DurabilityMode,
}
```

#### 14.2.3 Human-in-the-Loop (HITL)

```python
from langgraph.types import interrupt, Command

def execute_step_node(state):
    step = state["steps"][state["current_step_index"]]
    
    # 需要用户审批的步骤
    if step["approval_required"]:
        approved = interrupt(f"执行步骤 '{step['title']}' 吗？")
        if not approved:
            return {"errors": [f"用户拒绝步骤: {step['title']}"]}
    
    result = execute_step(step)
    return {"step_results": {step["id"]: result}}

# 恢复执行
graph.invoke(Command(resume=True), config)  # 用户确认后继续
```

**WindWave 对应设计**：
```rust
impl Orchestrator {
    fn execute_plan(&mut self, plan: Plan) {
        for step in &plan.steps {
            if step.approval_required {
                // 触发 HITL 中断
                self.interrupt(step.id, &step.title);
                // 将状态持久化到 Checkpointer
                self.checkpointer.save(&self.state);
                return;  // 等待用户输入
            }
            self.execute_step(step);
        }
    }

    fn resume(&mut self, decision: UserDecision) {
        // 从 Checkpointer 恢复状态，继续执行
        self.state = self.checkpointer.load(plan_id);
        self.execute_plan_from(self.state.current_step_index);
    }
}
```

#### 14.2.4 对 WindWave 的直接借鉴

| LangGraph 概念 | WindWave 对应 | 优先级 |
|---|---|---|
| StateGraph + 条件边 | Orchestrator 执行引擎 | **P0** |
| Durable Execution (checkpointing) | Plan 持久化 (G15) | **P0** |
| HITL `interrupt()`  | `approval_required` 字段 (G4) | **P0** |
| `thread_id` 游标 | `PlanId` 会话标识 | **P0** |
| `task()` 封装副作用 | ToolAdapter 的 execute() 幂等性 | **P1** |
| Event Streaming (v3) | `event_stream()` | **P1** |

---

### 14.3 GitHub MCP Server 深度解构

**项目**：[github/github-mcp-server](https://github.com/github/github-mcp-server) | 24K+ stars | MIT | 925 commits | Go

> **重要发现**：GitHub MCP 的公共仓库只包含 CLI 入口和文档，核心逻辑（`internal/ghmcp`、`pkg/github`、`pkg/http`）在内部仓库中。公共仓库仅 5 个 Go 文件，都是 CLI 脚手架代码。真正的 Server 实现是 GitHub 内部托管服务。

#### 14.3.1 双模架构：本地 + 远程

GitHub MCP 提供两种运行模式，覆盖不同场景：

**模式 A：远程托管（Remote Server）**
- URL: `https://api.githubcopilot.com/mcp/`
- 无需本地安装，零配置运行
- 支持 OAuth 2.1 自动认证（VS Code 1.101+）
- 额外工具：`create_pull_request_with_copilot`（调用 Copilot coding agent）
- 支持 GitHub Enterprise Cloud（`ghe.com`）

**模式 B：本地运行（Local Server）**
- Docker: `ghcr.io/github/github-mcp-server`
- 源码构建: `go build cmd/github-mcp-server`
- 支持 stdio 和 HTTP 两种传输
- 支持 GitHub Enterprise Server（`--gh-host` 标志）

```go
// cmd/github-mcp-server/main.go — CLI 入口
var (
    stdioCmd = &cobra.Command{
        Use: "stdio",  // stdio 传输
        RunE: func(_ *cobra.Command, _ []string) error {
            return ghmcp.RunStdioServer(stdioServerConfig)
        },
    }
    httpCmd = &cobra.Command{
        Use: "http",   // HTTP 传输
        RunE: func(_ *cobra.Command, _ []string) error {
            return ghhttp.RunHTTPServer(httpConfig)
        },
    }
)
```

**对 WindWave 的启示**：每个 ToolAdapter 应支持两种运行模式：
- **本地模式**：直接进程内调用（如 Blender Adapter 通过 Python subprocess）
- **远程模式**：HTTP/WebSocket 连接（如 Figma Adapter 通过 REST API）

#### 14.3.2 工具集（Toolset）机制的完整设计

**18 个工具集，每个独立 URL**：

| Toolset | 路径 | 功能 |
|---|---|---|
| `all` | `/mcp/` | 所有可用工具（默认） |
| `actions` | `/mcp/x/actions` | CI/CD 工作流 |
| `code_security` | `/mcp/x/code_security` | 代码扫描 |
| `copilot` | `/mcp/x/copilot` | Copilot 编码代理 |
| `copilot_spaces` | `/mcp/x/copilot_spaces` | Copilot Spaces（仅远程） |
| `dependabot` | `/mcp/x/dependabot` | 依赖更新 |
| `discussions` | `/mcp/x/discussions` | 讨论管理 |
| `gists` | `/mcp/x/gists` | Gist 管理 |
| `git` | `/mcp/x/git` | 底层 Git 操作 |
| `github_support_docs_search` | `/mcp/x/github_support_docs_search` | 文档搜索（仅远程） |
| `issues` | `/mcp/x/issues` | Issue 管理 |
| `labels` | `/mcp/x/labels` | 标签管理 |
| `notifications` | `/mcp/x/notifications` | 通知管理 |
| `orgs` | `/mcp/x/orgs` | 组织管理 |
| `projects` | `/mcp/x/projects` | 项目管理 |
| `pull_requests` | `/mcp/x/pull_requests` | PR 管理 |
| `repos` | `/mcp/x/repos` | 仓库管理 |
| `secret_protection` | `/mcp/x/secret_protection` | 密钥扫描 |
| `security_advisories` | `/mcp/x/security_advisories` | 安全公告 |
| `stargazers` | `/mcp/x/stargazers` | Star 管理 |
| `users` | `/mcp/x/users` | 用户管理 |

**三层过滤机制**：

```
第 1 层 — URL 路径过滤（粗粒度）
  /mcp/x/issues          → 仅 issues 工具集
  /mcp/x/issues/readonly → issues 工具集 + 只读

第 2 层 — Header 过滤（中粒度）
  X-MCP-Toolsets: repos,issues   → 多个工具集组合
  X-MCP-Tools: get_file_contents,issue_read  → 单个工具精确控制
  X-MCP-Readonly: true           → 全局只读模式

第 3 层 — 排除过滤（细粒度）
  X-MCP-Exclude-Tools: delete_repo,force_push  → 黑名单排除
```

**关键 CLI 标志**（来自 `main.go` 源码）：

```go
rootCmd.PersistentFlags().StringSlice("toolsets", nil, "Comma-separated list of toolsets")
rootCmd.PersistentFlags().StringSlice("tools", nil, "Specific tools to enable")
rootCmd.PersistentFlags().StringSlice("exclude-tools", nil, "Tools to disable")
rootCmd.PersistentFlags().Bool("read-only", false, "Restrict to read-only")
rootCmd.PersistentFlags().Bool("insiders", false, "Enable insiders features")
rootCmd.PersistentFlags().Bool("lockdown-mode", false, "Enable lockdown mode")
rootCmd.PersistentFlags().Duration("repo-access-cache-ttl", 5*time.Minute, "Cache TTL")
```

**对 WindWave 的直接应用**：

```rust
// WindWave 的 ToolAdapter 配置
pub struct AdapterConfig {
    /// 默认启用的工具组（对应默认 toolsets）
    pub default_capabilities: Vec<CapabilityGroup>,
    /// 可选启用的工具组（需要手动开启）
    pub optional_capabilities: Vec<CapabilityGroup>,
    /// 黑名单排除（对应 exclude-tools）
    pub excluded_commands: Vec<String>,
    /// 全局只读模式
    pub read_only: bool,
    /// 高级功能开关（对应 insiders）
    pub experimental: bool,
}

pub struct CapabilityGroup {
    pub name: String,           // 如 "scene_editing", "material_management"
    pub description: String,
    pub commands: Vec<ToolCommand>,
    pub default_enabled: bool,  // 是否默认启用
}
```

#### 14.3.3 配置体系深度分析

**配置来源优先级**（Viper 框架）：

```
1. 命令行标志（--toolsets repos,issues）
2. 环境变量（GITHUB_TOOLSETS=repos,issues）
3. 默认值（default toolsets）
```

**关键配置项**：

| 配置项 | 环境变量 | 默认值 | 说明 |
|---|---|---|---|
| `toolsets` | `GITHUB_TOOLSETS` | `nil`（使用默认） | 工具集列表 |
| `tools` | `GITHUB_TOOLS` | `nil`（全部启用） | 精确工具列表 |
| `exclude-tools` | `GITHUB_EXCLUDE_TOOLS` | `nil` | 黑名单 |
| `read-only` | `GITHUB_READ_ONLY` | `false` | 只读模式 |
| `lockdown-mode` | `GITHUB_LOCKDOWN_MODE` | `false` | 锁定模式 |
| `insiders` | `GITHUB_INSIDERS` | `false` | 实验功能 |
| `gh-host` | `GITHUB_HOST` | `""` | 自定义 GitHub 实例 |
| `content-window-size` | `GITHUB_CONTENT_WINDOW_SIZE` | `5000` | 内容窗口大小 |
| `repo-access-cache-ttl` | `GITHUB_REPO_ACCESS_CACHE_TTL` | `5m` | 缓存 TTL |
| `enable-command-logging` | `GITHUB_ENABLE_COMMAND_LOGGING` | `false` | 命令日志 |
| `log-file` | `GITHUB_LOG_FILE` | `""` | 日志文件路径 |

**关键设计决策**：
- `toolsets` 为 `nil` 时使用默认工具集，而非空列表（避免空列表被误认为"无工具"）
- `tools` 和 `exclude-tools` 可以同时使用，`exclude-tools` 优先级更高
- `read-only` 模式覆盖所有工具集，即使工具集本身支持写入

**对 WindWave 的配置设计**：

```rust
// WindWave 的配置层级
// 1. 全局配置（~/.windwave/config.toml）
// 2. 项目配置（.windwave/config.toml）
// 3. 环境变量（WINDWAVE_*）
// 4. 命令行参数（--adapter-config）

pub struct WindWaveConfig {
    pub adapters: HashMap<String, AdapterConfig>,
    pub global: GlobalConfig,
}

pub struct GlobalConfig {
    pub read_only: bool,
    pub content_window_size: usize,  // 默认 5000
    pub command_logging: bool,
    pub log_file: Option<PathBuf>,
}
```

#### 14.3.4 生产级架构决策（从演讲和代码中提取）

**1. 无状态架构 + 令牌传递**

```
客户端 → Authorization: Bearer <token> → MCP Server → GitHub API
```

Server 本身不存储任何用户状态，每次请求携带完整认证信息。这简化了水平扩展和故障恢复。

**2. 内容窗口管理**

`content-window-size` 参数控制返回给 LLM 的内容大小：
- 默认 5000 字符
- 防止单次 tool call 返回过多内容撑爆上下文窗口
- 可配置以适应不同模型（GPT-4 128K vs Claude 200K）

**3. 锁定模式（Lockdown Mode）**

隐藏来自无 push 权限用户的公开 issue 详情，防止敏感信息泄露。

**4. 仓库访问缓存**

`repo-access-cache-ttl`（默认 5 分钟）缓存仓库访问权限检查结果，减少 API 调用。

**5. 实验功能通道（Insiders）**

通过 `/mcp/insiders` URL 或 `X-MCP-Insiders` header 开启实验功能，允许快速迭代而不影响稳定版用户。

#### 14.3.5 对 WindWave 的直接借鉴（更新版）

| # | GitHub MCP 概念 | WindWave 对应 | 具体实现 | 优先级 |
|---|---|---|---|---|
| 1 | 18 个工具集 × 独立 URL | ToolAdapter × capabilities 分组 | 每个 Adapter 注册时可声明多个 CapabilityGroup | **P0** |
| 2 | 三层过滤（URL/Header/Exclude） | 三层过滤（配置/权限/黑名单） | `AdapterConfig` + `GlobalConfig` + `ToolDock` 开关 | **P0** |
| 3 | Remote Server 模式 | 云端 ToolAdapter | Figma、GitHub 等远程工具的适配器 | **P1** |
| 4 | 双模传输（stdio + HTTP） | 本地 + 远程双模 | `AdapterTransport::Local` vs `AdapterTransport::Remote` | **P1** |
| 5 | `content-window-size` 控制 | 上下文窗口管理 | `ToolContext::window_size` 限制返回内容 | **P1** |
| 6 | Viper 多层配置 | config crate 多层配置 | 全局 → 项目 → 环境变量 → CLI | **P1** |
| 7 | OAuth 2.1 + PKCE | 凭证安全存储（G19） | `CredentialStore` trait | **P1** |
| 8 | Lockdown 模式 | 敏感操作确认 | 高风险命令需要二次确认 | **P2** |
| 9 | Insiders 实验通道 | 实验功能开关 | `AdapterConfig::experimental` | **P2** |
| 10 | `repo-access-cache-ttl` | 权限检查缓存 | `PermissionCache` with TTL | **P2** |
| 11 | `tool-search` CLI 工具 | 工具发现 CLI | `windwave tool list --adapter blender` | **P2** |
| 12 | 评估框架 | G13 测试策略 | Mandrel MCP Test Harness 集成 | **P2** |

---

### 14.4 CodePrism — Rust MCP Server 参考实现

**项目**：[dragonscale-ai/codeprism](https://github.com/dragonscale-ai/codeprism) | 2K+ stars | MIT | Rust | 28K SLoC

> **关键特性**：100% AI 生成的 Rust 代码库，是 AI-first 工程实践的实验项目。不接收人类编写的代码贡献。

#### 14.4.1 三层架构

CodePrism 的架构与 WindWave 高度相似——都是三层设计：

```
┌─────────────────────────────────────────────────┐
│              MCP Protocol Layer                  │
│  JSON-RPC 2.0 over stdin/stdout                 │
│  - Capability negotiation                        │
│  - Tool/resource request routing                 │
│  - Structured error handling                     │
│  - Real-time notifications                       │
└────────────────────┬────────────────────────────┘
                     │
┌────────────────────┴────────────────────────────┐
│           Analysis Tools Engine                  │
│  23 production-ready tools                       │
│  - Plugin-based tool system                      │
│  - Parallel execution for batch operations       │
│  - Caching for expensive computations            │
│  - Result aggregation and formatting             │
└────────────────────┬────────────────────────────┘
                     │
┌────────────────────┴────────────────────────────┐
│         Code Intelligence Engine                 │
│  - Parser Framework (Tree-sitter)                │
│  - Universal AST Graph (DashMap)                 │
│  - Symbol Resolution (cross-file/language)       │
│  - Incremental Updates                           │
└─────────────────────────────────────────────────┘
```

#### 14.4.2 核心数据结构（可直接借鉴）

**图存储结构**：

```rust
pub struct CodeGraph {
    nodes: DashMap<NodeId, Node>,       // 并发安全
    edges: DashMap<EdgeId, Edge>,
    indexes: GraphIndexes,
}

pub struct GraphIndexes {
    by_file: HashMap<PathBuf, Vec<NodeId>>,
    by_symbol: HashMap<String, Vec<NodeId>>,
    by_type: HashMap<NodeKind, Vec<NodeId>>,
    dependencies: HashMap<NodeId, Vec<NodeId>>,
}
```

**设计决策**：
- **DashMap** 而非 `RwLock<HashMap>` — 更高并发性能
- **多级索引** — 按文件、符号名、类型、依赖关系分别建索引
- **LRU 缓存** — 昂贵分析结果缓存
- **可选持久化** — 未来支持快速启动

**通用 AST 节点类型**：

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeKind {
    Module,      // 文件/模块级
    Function,    // 函数/方法
    Class,       // 类/类型
    Variable,    // 变量/常量
    Import,      // 导入/包含语句
    Call,        // 函数调用
    Reference,   // 符号引用
}
```

**可插拔语言解析器 trait**：

```rust
pub trait LanguageParser: Send + Sync {
    fn parse_file(&self, context: ParseContext) -> Result<ParseResult>;
    fn supported_extensions(&self) -> &[&str];
    fn language_name(&self) -> &str;
    fn incremental_update(&self, old_tree: &Tree, edit: &InputEdit) -> Result<Tree>;
}
```

**可扩展分析工具 trait**：

```rust
pub trait AnalysisTool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> serde_json::Value;  // JSON Schema
    fn execute(&self, graph: &CodeGraph, params: &serde_json::Value) -> Result<ToolResult>;
}
```

**对 WindWave 的直接映射**：

```rust
// WindWave 的 ToolAdapter trait（借鉴 AnalysisTool）
pub trait ToolAdapter: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    /// 返回 capabilities（每个包含多个 commands）
    fn capabilities(&self) -> Vec<CapabilityGroup>;
    /// 执行单个命令
    fn execute(&self, command: &str, params: &serde_json::Value) -> Result<ToolResult>;
    /// 返回当前状态
    fn status(&self) -> AdapterStatus;
}

// WindWave 的图存储（借鉴 CodeGraph）
pub struct ToolGraph {
    adapters: DashMap<AdapterId, AdapterNode>,
    commands: DashMap<CommandId, CommandNode>,
    edges: DashMap<EdgeId, ToolEdge>,
    indexes: ToolGraphIndexes,
}
```

#### 14.4.3 性能指标（实测数据）

| 操作 | 目标延迟 | 实测性能 |
|---|---|---|
| 仓库扫描（1K 文件） | < 2s | 1.2s |
| 简单工具查询 | < 100ms | 45ms avg |
| 复杂分析 | < 500ms | 320ms avg |
| 文件变更更新 | < 250ms | 180ms avg |

**优化策略**（可直接借鉴）：
1. **并行解析** — 初始化时并行处理文件
2. **增量更新** — 文件变更时仅重新解析变更部分
3. **多级缓存** — 解析结果 → 分析结果 → 格式化响应
4. **图索引优化** — 通过多级索引加速查询
5. **批量操作** — 多个相关查询合并处理

#### 14.4.4 Mandrel MCP Test Harness

**项目**：`mandrel-mcp-th`（`moth` CLI）| crates.io 发布

**核心理念**：用 YAML 声明式定义 MCP 服务器测试，自动验证协议合规性。

```yaml
# 测试规范示例
name: "Filesystem MCP Server"
server:
  command: "node"
  args: ["filesystem-server.js"]
  transport: "stdio"
  startup_timeout_seconds: 10
tools:
  - name: "read_file"
    description: "Read file contents"
    tests:
      - name: "read_existing_file"
        input:
          path: "/allowed/path/test.txt"
        expected:
          error: false
          schema:
            type: object
            required: ["content"]
```

**架构**：
```
┌─────────────┐    ┌─────────────┐    ┌─────────────┐
│   Config    │───▶│   Client    │───▶│  Executor   │
│  (YAML)     │    │ (MCP/rmcp)  │    │ (Test Run)  │
└─────────────┘    └─────────────┘    └─────────────┘
       │                   │                   │
       ▼                   ▼                   ▼
┌─────────────┐    ┌─────────────┐    ┌─────────────┐
│ Validation  │    │   Server    │    │  Reporting  │
│  (Schema)   │    │ (Process)   │    │ (JSON/HTML) │
└─────────────┘    └─────────────┘    └─────────────┘
```

**输出格式**：HTML、JSON、JUnit XML
**CI/CD 集成**：GitHub Actions、GitLab CI、Jenkins

**对 WindWave 的直接应用**：
- 每个 ToolAdapter 编写 YAML 测试规范
- CI/CD 中用 `moth` 验证 MCP 协议合规性
- 这是 G13（测试策略）的最佳实践参考

#### 14.4.5 安全与隔离模型

CodePrism 的安全设计值得 WindWave 效仿：

```
- 只读访问指定仓库目录
- 路径验证防止目录遍历攻击
- 内存和 CPU 资源限制
- 解析器失败不导致服务器崩溃（错误隔离）
- 默认不持久化代码内容
- 本地处理，无外部网络调用
- 最小权限原则
- 所有文件路径和参数输入消毒
```

**WindWave 的安全增强**：

```rust
pub struct SandboxConfig {
    pub allowed_paths: Vec<PathBuf>,       // 允许访问的路径
    pub read_only: bool,                    // 只读模式
    pub max_memory_mb: usize,              // 内存限制（默认 2048）
    pub max_cpu_seconds: u64,              // CPU 时间限制
    pub max_file_size_mb: usize,           // 单文件大小限制
    pub network_access: NetworkPolicy,      // 网络访问策略
    pub command_allowlist: Vec<String>,     // 命令白名单
    pub command_blocklist: Vec<String>,     // 命令黑名单
}

pub enum NetworkPolicy {
    None,           // 禁止所有网络访问
    LocalOnly,      // 仅 localhost
    AllowList(Vec<String>),  // 白名单域名
    All,            // 允许所有（不推荐）
}
```

#### 14.4.6 对 WindWave 的直接借鉴（更新版）

| # | CodePrism 概念 | WindWave 对应 | 具体实现 | 优先级 |
|---|---|---|---|---|
| 1 | `AnalysisTool` trait | `ToolAdapter` trait | 统一的适配器接口 | **P0** |
| 2 | `LanguageParser` trait | `ToolAdapter` 可插拔设计 | 新适配器只需实现 trait | **P0** |
| 3 | `CodeGraph` (DashMap) | `ToolGraph` 并发存储 | 适配器注册和命令索引 | **P1** |
| 4 | GraphIndexes 多级索引 | 多级命令索引 | 按适配器、能力组、命令类型索引 | **P1** |
| 5 | Mandrel MCP Test Harness | G13 测试策略 | YAML 测试规范 + CI/CD 集成 | **P1** |
| 6 | 并行解析 + 增量更新 | 并行适配器发现 + 增量注册 | 启动时并行扫描，运行时动态注册/注销 | **P1** |
| 7 | 多级缓存（解析→分析→响应） | 命令结果缓存 | `CommandCache` with TTL | **P2** |
| 8 | 安全沙箱（路径验证、资源限制） | ToolAdapter 安全沙箱 | `SandboxConfig` 限制每个适配器 | **P2** |
| 9 | Universal AST 统一节点 | 统一命令结果格式 | `ToolResult` 标准化 | **P2** |
| 10 | 语言扩展（WASM 插件） | 第三方适配器市场 | 社区贡献的 ToolAdapter | **P3** |

---

### 14.5 综合结论：直接可复用的设计模式

#### 14.5.1 完整设计模式汇总（更新版）

| # | 模式 | 来源 | 直接应用于 WindWave | Phase |
|---|---|---|---|---|
| 1 | **Domain Symmetry** | Unity MCP | ToolAdapter::execute() ↔ 目标工具命令一一对应 | P0 |
| 2 | **工具分组 + 三层过滤** | GitHub MCP | ToolAdapter capabilities 分组 + ToolDock + 配置过滤 | P0 |
| 3 | **StateGraph + 条件路由** | LangGraph | Orchestrator 执行引擎核心 | P0 |
| 4 | **Durable Execution (checkpointing)** | LangGraph | PlanId + Checkpointer 持久化 | P0 |
| 5 | **HITL `interrupt()`** | LangGraph | PlanStep.approval_required | P0 |
| 6 | **`ToolAdapter` trait（借鉴 AnalysisTool）** | CodePrism | 统一的适配器接口 | P0 |
| 7 | **双模传输（本地 + 远程）** | GitHub MCP | `AdapterTransport::Local` vs `AdapterTransport::Remote` | P1 |
| 8 | **多层配置体系（Viper 模式）** | GitHub MCP | 全局 → 项目 → 环境变量 → CLI | P1 |
| 9 | **DashMap 并发图存储** | CodePrism | ToolGraph 适配器注册和命令索引 | P1 |
| 10 | **Mandrel MCP Test Harness** | CodePrism | WindWave 的 MCP 合规测试 | P1 |
| 11 | **`thread_id` = `PlanId`** | LangGraph | 会话标识与恢复 | P1 |
| 12 | **`toolParams` 验证模式** | Unity MCP | ToolCommand 参数验证 | P1 |
| 13 | **OAuth 2.1 + PKCE** | GitHub MCP | 凭证安全存储 | P1 |
| 14 | **内容窗口管理（content-window-size）** | GitHub MCP | 上下文膨胀控制 | P1 |
| 15 | **安全沙箱（SandboxConfig）** | CodePrism | 每个 ToolAdapter 的资源限制 | P2 |
| 16 | **多级缓存策略** | CodePrism | 命令结果缓存 | P2 |
| 17 | **Insiders 实验通道** | GitHub MCP | 实验功能开关 | P2 |
| 18 | **Lockdown 模式** | GitHub MCP | 敏感操作二次确认 | P2 |
| 19 | **WASM 插件扩展** | CodePrism | 第三方适配器市场 | P3 |

#### 14.5.2 关键架构决策汇总

| 决策 | 选择 | 理由 | 来源 |
|---|---|---|---|
| 适配器接口 | `ToolAdapter` trait (Send + Sync) | 统一接口，可插拔设计 | CodePrism `AnalysisTool` |
| 并发存储 | `DashMap` 而非 `RwLock<HashMap>` | 更高并发性能 | CodePrism `CodeGraph` |
| 工具过滤 | 三层过滤（配置/权限/黑名单） | 灵活可控，避免上下文膨胀 | GitHub MCP toolsets |
| 配置管理 | 多层配置（全局→项目→环境→CLI） | 灵活覆盖，生产就绪 | GitHub MCP Viper 模式 |
| 传输模式 | 本地 + 远程双模 | 覆盖所有工具类型 | GitHub MCP stdio + HTTP |
| 执行引擎 | StateGraph + Checkpointer | 持久化、可恢复、可中断 | LangGraph |
| 测试策略 | Mandrel MCP Test Harness | MCP 协议合规验证 | CodePrism `moth` |
| 安全模型 | `SandboxConfig` 每个适配器 | 最小权限，错误隔离 | CodePrism 安全设计 |

#### 14.5.3 代码哲学教训（更新版）

来自四个项目的共同教训：

1. **避免过早抽象**（Unity MCP）— Phase 1 不要过度设计 ToolAdapter trait，先让 2-3 个适配器跑起来再抽象
2. **Domain Symmetry**（Unity MCP）— 每个 ToolAdapter 的命令应该与目标工具的原生命令语义一致
3. **删除而非弃用**（Unity MCP）— 不保留向后兼容 shim
4. **测试覆盖要求**（Unity MCP）— 每个新适配器必须有对应的测试
5. **无状态设计**（GitHub MCP）— 每次请求携带完整认证信息，简化扩展
6. **实验通道**（GitHub MCP）— Insiders 模式允许快速迭代而不影响稳定用户
7. **并发优先**（CodePrism）— DashMap 而非 RwLock，从第一天就考虑并发
8. **AI-first 工程**（CodePrism）— 100% AI 生成的代码库证明了 AI 驱动开发的可行性
9. **持久化执行**（LangGraph）— checkpointing 是复杂工作流的基础，不是可选的附加功能
10. **HITL 内置**（LangGraph）— 审批流应该是执行引擎的内置能力，而非外部附加

#### 14.5.4 实施路线图（基于研究结论）

```
Phase 0 (当前): 基础工具适配器
  ├── Blender MCP Adapter (本地模式)
  ├── Godot MCP Adapter (本地模式)
  └── VS Code MCP Adapter (本地模式)

Phase 1 (P0): 核心架构
  ├── ToolAdapter trait 标准化 (借鉴 CodePrism AnalysisTool)
  ├── ToolOrchestrator + StateGraph 执行引擎 (借鉴 LangGraph)
  ├── CapabilityGroup 工具分组 (借鉴 GitHub MCP toolsets)
  ├── 三层过滤机制 (借鉴 GitHub MCP 三层过滤)
  └── PlanId + Checkpointer 持久化 (借鉴 LangGraph)

Phase 2 (P1): 生产就绪
  ├── 双模传输 (本地 + 远程)
  ├── 多层配置体系 (Viper 模式)
  ├── DashMap 并发图存储 (借鉴 CodePrism CodeGraph)
  ├── Mandrel MCP Test Harness 集成
  ├── 内容窗口管理 (context-window-size)
  └── OAuth 2.1 + PKCE 凭证存储

Phase 3 (P2): 增强功能
  ├── 安全沙箱 (SandboxConfig)
  ├── 多级缓存策略
  ├── Insiders 实验通道
  ├── Lockdown 敏感操作确认
  └── 命令结果缓存

Phase 4 (P3): 生态扩展
  ├── WASM 插件扩展
  ├── 第三方适配器市场
  └── 社区贡献的 ToolAdapter
```

---

### 14.6 MCP 协议规范深度解析（2025-06-18）

> **规范地址**：[modelcontextprotocol.io/specification/2025-06-18](https://modelcontextprotocol.io/specification/2025-06-18)
> **权威来源**：基于 TypeScript schema [schema.ts](https://github.com/modelcontextprotocol/specification/blob/main/schema/2025-06-18/schema.ts)

#### 14.6.1 协议生命周期

MCP 定义了严格的连接生命周期，WindWave 的 ToolAdapter 必须遵循：

```
┌─────────────────────────────────────────────────────┐
│                MCP 连接生命周期                       │
│                                                       │
│  ┌──────────┐    ┌──────────┐    ┌──────────┐       │
│  │ 1. 初始化 │───▶│ 2. 运行  │───▶│ 3. 关闭  │       │
│  │ 能力协商  │    │ 正常通信  │    │ 优雅终止  │       │
│  └──────────┘    └──────────┘    └──────────┘       │
└─────────────────────────────────────────────────────┘
```

**初始化阶段（必须首先执行）**：

1. 客户端发送 `initialize` 请求：
   - `protocolVersion`: 协议版本（如 `"2025-06-18"`）
   - `capabilities`: 客户端能力（`roots`、`sampling`、`elicitation`）
   - `clientInfo`: 实现信息（name、title、version）

2. 服务器响应：
   - `protocolVersion`: 确认或协商版本
   - `capabilities`: 服务器能力（`tools`、`resources`、`prompts`、`logging`）
   - `serverInfo`: 实现信息
   - `instructions`: 可选的服务器使用说明

3. 客户端发送 `notifications/initialized` 通知，进入运行阶段

**关键约束**：
- 客户端在收到 `initialize` 响应前，**只能**发送 ping
- 服务器在收到 `initialized` 通知前，**只能**发送 ping 和 logging
- 版本不匹配时，服务器应返回另一个支持的版本，客户端可选择断开

**对 WindWave 的直接应用**：

```rust
// WindWave 的 ToolAdapter 初始化流程
impl ToolAdapter for BlenderAdapter {
    async fn initialize(&self) -> Result<AdapterCapabilities> {
        // 1. 检查 Blender 是否运行
        // 2. 协商能力（支持的命令列表）
        // 3. 返回能力声明
        Ok(AdapterCapabilities {
            protocol_version: "2025-06-18".into(),
            tools: ToolCapability {
                list_changed: true,  // 支持动态工具列表变更
            },
            server_info: ServerInfo {
                name: "windwave-blender-adapter".into(),
                title: "Blender Adapter for WindWave".into(),
                version: env!("CARGO_PKG_VERSION").into(),
            },
            instructions: Some("Use this adapter to control Blender...".into()),
        })
    }
}
```

#### 14.6.2 工具（Tools）规范——完整数据模型

**Tool 定义**（JSON Schema）：

```typescript
interface Tool {
  name: string;              // 唯一标识符（必需）
  title?: string;            // 人类可读显示名称
  description?: string;      // 功能描述
  inputSchema: {             // JSON Schema 定义参数（必需）
    type: "object";
    properties?: { ... };
    required?: string[];
  };
  outputSchema?: { ... };    // 可选的输出结构定义
  annotations?: {            // 可选的工具行为注解
    title?: string;
    readOnlyHint?: boolean;       // 提示此工具为只读
    destructiveHint?: boolean;    // 提示此工具具有破坏性
    idempotentHint?: boolean;     // 提示此工具幂等
    openWorldHint?: boolean;      // 提示此工具连接外部世界
  };
}
```

**关键注解（Annotations）——WindWave 必须实现**：

| 注解 | 含义 | WindWave 应用 |
|---|---|---|
| `readOnlyHint` | 工具不修改任何状态 | 跳过审批流程 |
| `destructiveHint` | 工具可能删除/破坏数据 | **强制二次确认** |
| `idempotentHint` | 重复调用产生相同结果 | 支持自动重试 |
| `openWorldHint` | 工具连接外部系统 | 网络访问策略检查 |

**工具调用结果**支持 6 种内容类型：

1. **Text Content** — 纯文本结果
2. **Image Content** — base64 编码图片（含 mimeType）
3. **Audio Content** — base64 编码音频
4. **Resource Links** — 资源 URI 引用（可订阅/获取）
5. **Embedded Resources** — 内嵌资源内容
6. **Structured Content** — 结构化 JSON 对象（含 `outputSchema` 验证）

**两种错误处理模式**：

```json
// 模式 1: 协议错误（JSON-RPC 标准错误）
{
  "error": {
    "code": -32602,
    "message": "Unknown tool: invalid_tool_name"
  }
}

// 模式 2: 工具执行错误（isError: true）
{
  "result": {
    "content": [{ "type": "text", "text": "Failed: API rate limit exceeded" }],
    "isError": true
  }
}
```

**对 WindWave 的 ToolCommand 设计**：

```rust
pub struct ToolCommand {
    pub name: String,
    pub title: Option<String>,
    pub description: String,
    pub input_schema: serde_json::Value,  // JSON Schema
    pub output_schema: Option<serde_json::Value>,
    pub annotations: ToolAnnotations,
}

pub struct ToolAnnotations {
    pub read_only_hint: bool,
    pub destructive_hint: bool,
    pub idempotent_hint: bool,
    pub open_world_hint: bool,
}

pub enum ToolResult {
    Text { text: String },
    Image { data: Vec<u8>, mime_type: String },
    Audio { data: Vec<u8>, mime_type: String },
    ResourceLink { uri: String, name: String, mime_type: String },
    EmbeddedResource { uri: String, mime_type: String, content: String },
    Structured { data: serde_json::Value, text_fallback: String },
}

pub struct ToolCallResult {
    pub content: Vec<ToolResult>,
    pub is_error: bool,
    pub structured_content: Option<serde_json::Value>,
}
```

#### 14.6.3 传输层（Transports）——stdio + Streamable HTTP

**stdio 传输**（本地适配器首选）：

```
Client → 启动 Server 子进程 → stdin/stdout JSON-RPC 消息
- 消息以换行符分隔，不得包含嵌入换行
- Server 可写 stderr 用于日志
- 关闭：Client 关闭 stdin → 等待退出 → SIGTERM → SIGKILL
```

**Streamable HTTP 传输**（远程适配器）+ 取代了旧的 HTTP+SSE：

```
┌──────────────────────────────────────────────────────┐
│             Streamable HTTP 传输                      │
│                                                        │
│  POST /mcp  → 发送 JSON-RPC 请求                      │
│  GET  /mcp  → 打开 SSE 流接收服务器消息                │
│                                                        │
│  关键特性:                                              │
│  - 会话管理: Mcp-Session-Id header                    │
│  - 可恢复性: Last-Event-ID header                     │
│  - 多连接: 同时维持多个 SSE 流                         │
│  - 协议版本: MCP-Protocol-Version header              │
│  - 向后兼容: 支持旧版 HTTP+SSE 传输                    │
└──────────────────────────────────────────────────────┘
```

**安全要求**：
- 服务器**必须**验证 `Origin` header 防止 DNS rebinding
- 本地运行时**应该**仅绑定 localhost（127.0.0.1）
- 服务器**应该**实现适当的认证

**对 WindWave 的传输设计**：

```rust
pub enum AdapterTransport {
    /// 本地子进程（stdio）—— Blender、Godot、VS Code
    Stdio {
        command: String,
        args: Vec<String>,
        env: HashMap<String, String>,
    },
    /// 远程 HTTP —— Figma、GitHub
    StreamableHttp {
        url: String,
        session_id: Option<String>,
        headers: HashMap<String, String>,
    },
    /// 自定义传输（WebSocket 等）
    Custom(Box<dyn CustomTransport>),
}
```

#### 14.6.4 能力协商

| 类别 | 能力 | 描述 | WindWave 对应 |
|---|---|---|---|
| Server | `tools` | 暴露可调用工具 | ToolAdapter 命令 |
| Server | `resources` | 提供可读资源 | 项目文件、场景数据 |
| Server | `prompts` | 提供提示模板 | 预设 Agent 指令 |
| Server | `logging` | 结构化日志 | 操作日志面板 |
| Server | `completions` | 参数自动补全 | 命令参数智能提示 |
| Client | `roots` | 文件系统根目录 | 项目根目录 |
| Client | `sampling` | LLM 采样请求 | Agent 递归调用 |
| Client | `elicitation` | 向用户请求额外信息 | 缺失参数弹窗 |

**子能力**：
- `listChanged`: 支持列表变更通知（prompts、resources、tools 均支持）
- `subscribe`: 支持订阅单个资源变更

#### 14.6.5 对 WindWave 的协议合规借鉴

| # | MCP 规范要求 | WindWave 实现 | 优先级 |
|---|---|---|---|
| 1 | 严格的初始化 → 运行 → 关闭生命周期 | `ToolAdapter::initialize()` → `execute()` → `shutdown()` | **P0** |
| 2 | Tool 定义含 `inputSchema` + `outputSchema` | `ToolCommand` 含 JSON Schema | **P0** |
| 3 | 工具注解（readOnlyHint、destructiveHint 等） | `ToolAnnotations` 驱动审批流 | **P0** |
| 4 | 两种错误模式（协议错误 + 执行错误） | `ToolCallResult::is_error` | **P0** |
| 5 | `tools/list` 支持分页 | 大量命令时的分页支持 | **P1** |
| 6 | `notifications/tools/list_changed` | 动态命令注册/注销 | **P1** |
| 7 | 6 种内容类型 | `ToolResult` enum 完整支持 | **P1** |
| 8 | Streamable HTTP 会话管理 | 远程适配器会话恢复 | **P1** |
| 9 | 能力协商（capabilities） | 适配器启动时自动协商 | **P1** |
| 10 | `completions` 参数自动补全 | 命令参数智能提示 | **P2** |
| 11 | `elicitation` 用户信息请求 | 缺失参数交互式收集 | **P2** |
| 12 | 自定义传输 | WebSocket 等自定义传输 | **P2** |

---

### 14.7 PlayCanvas MCP — 游戏引擎 MCP 集成参考

**项目**：[playcanvas/editor-mcp-server](https://github.com/playcanvas/editor-mcp-server) | 329 stars | TypeScript | Node.js

> **关键价值**：PlayCanvas MCP 是最接近 WindWave 场景的游戏引擎 MCP 集成——它是 Unity MCP 之外唯一一个 Web 游戏引擎的 MCP 实现。

#### 14.7.1 架构模式

```
┌─────────────────────────────────────────────────────────┐
│                   AI Host (Claude/Cursor)                │
│                       MCP Client                         │
└──────────────────────────┬──────────────────────────────┘
                           │ MCP Protocol (stdio)
┌──────────────────────────┴──────────────────────────────┐
│           PlayCanvas Editor MCP Server (Node.js)         │
│  ┌──────────────────────────────────────────────────┐   │
│  │  Tools:                                           │   │
│  │  ├── Entity Management (list/create/delete/dup)   │   │
│  │  ├── Asset Control (list/create/delete/instantiate)│   │
│  │  ├── Scene Configuration (query/modify settings)  │   │
│  │  ├── Script Management (parse/modify scripts)     │   │
│  │  ├── Material Management (adjust materials)       │   │
│  │  └── Store Integration (search/download assets)   │   │
│  └──────────────────┬───────────────────────────────┘   │
└─────────────────────┼───────────────────────────────────┘
                      │ WebSocket
┌─────────────────────┴───────────────────────────────────┐
│         Chrome Extension (PlayCanvas Editor)             │
│  ┌──────────────────────────────────────────────────┐   │
│  │  WebSocket Client → PlayCanvas Editor API         │   │
│  └──────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
```

**关键架构决策**：
- MCP Server 通过 **Chrome Extension** 桥接到 PlayCanvas Editor
- 使用 **WebSocket** 进行实时通信（而非 REST API）
- 需要用户在 Editor 中**显式连接**到 Server
- 仅支持 Claude Sonnet（3.5/3.7），推荐 Pro 计划

**工具分类**：

| 类别 | 工具 | 读/写 |
|---|---|---|
| Entity Management | `list_entities`, `create_entities`, `delete_entities`, `duplicate_entities`, `modify_entities`, `reparent_entity` | 读写 |
| Asset Control | `list_assets`, `create_assets`, `delete_assets`, `instantiate_template_assets` | 读写 |
| Scene Configuration | `query_scene_settings`, `modify_scene_settings` | 读写 |
| Script Management | `parse_script`, `modify_script` | 读写 |
| Material Management | `adjust_material_properties` | 写 |
| Store Integration | `search_store_assets`, `retrieve_store_asset`, `download_store_asset` | 读 |

#### 14.7.2 对 WindWave 的关键启示

**1. 浏览器扩展桥接模式**

PlayCanvas 是 Web 编辑器，无法直接通过子进程控制。解决方案是 Chrome Extension + WebSocket：

```
WindWave 的类似场景：
- Figma（Web 应用）→ 需要 Chrome Extension 或 REST API 桥接
- Unity Web Editor → 类似模式
```

**2. 工具上下文敏感性问题**

PlayCanvas MCP 的实际问题：工具列表过大时，会超出免费版 Claude 的上下文限制。这验证了 GitHub MCP 的 toolsets 三层过滤的必要性。

```
WindWave 的教训：
- 每个 ToolAdapter 的默认工具集必须精简
- 提供工具过滤机制（借鉴 GitHub MCP）
- 考虑按编辑上下文动态切换可见工具
```

**3. 显式连接确认**

用户必须在 Editor 中显式点击"连接"按钮，MCP Server 才能操控编辑器。这是一种安全设计：

```rust
// WindWave 的类似设计
impl ToolAdapter {
    /// 需要用户显式授权后才能执行命令
    fn requires_explicit_connect(&self) -> bool {
        // Blender: 需要用户在 Blender 中启用 MCP 插件
        // Figma: 需要用户授权 OAuth
        // Terminal: 需要用户确认工作目录
        true
    }
}
```

**4. WebSocket 实时通信**

PlayCanvas 使用 WebSocket 而非 REST 进行实时通信，因为游戏编辑器需要低延迟：

```
WindWave 的传输选择：
- Blender: Python subprocess + stdout/stdin（类似 stdio）
- Godot: GDScript subprocess
- Figma: REST API（非实时）
- Terminal: PTY（伪终端）
- 未来实时协作: WebSocket
```

#### 14.7.3 对 WindWave 的直接借鉴

| # | PlayCanvas MCP 概念 | WindWave 对应 | 优先级 |
|---|---|---|---|
| 1 | 浏览器扩展桥接 | Figma Adapter 的 Chrome Extension 模式 | **P1** |
| 2 | WebSocket 实时通信 | 实时协作场景的 WebSocket 传输 | **P2** |
| 3 | 显式连接确认 | 每个 Adapter 的授权流程 | **P1** |
| 4 | 工具分类（Entity/Asset/Scene/Script） | CapabilityGroup 分组 | **P0** |
| 5 | 上下文限制问题 | 三层工具过滤（借鉴 GitHub MCP） | **P0** |
| 6 | Store 集成 | 资源市场/素材库集成 | **P3** |

---

### 14.8 更新版综合结论

#### 14.8.1 完整设计模式汇总（最终版）

| # | 模式 | 来源 | 直接应用于 WindWave | Phase |
|---|---|---|---|---|
| 1 | **Domain Symmetry** | Unity MCP | ToolAdapter::execute() ↔ 目标工具命令一一对应 | P0 |
| 2 | **工具分组 + 三层过滤** | GitHub MCP | ToolAdapter capabilities 分组 + ToolDock + 配置过滤 | P0 |
| 3 | **StateGraph + 条件路由** | LangGraph | Orchestrator 执行引擎核心 | P0 |
| 4 | **Durable Execution (checkpointing)** | LangGraph | PlanId + Checkpointer 持久化 | P0 |
| 5 | **HITL `interrupt()`** | LangGraph | PlanStep.approval_required | P0 |
| 6 | **`ToolAdapter` trait（借鉴 AnalysisTool）** | CodePrism | 统一的适配器接口 | P0 |
| 7 | **MCP 初始化生命周期** | MCP Spec | `initialize()` → `execute()` → `shutdown()` | P0 |
| 8 | **Tool 注解（readOnlyHint/destructiveHint）** | MCP Spec | `ToolAnnotations` 驱动审批流 | P0 |
| 9 | **双模传输（本地 + 远程）** | GitHub MCP | `AdapterTransport::Stdio` vs `AdapterTransport::StreamableHttp` | P1 |
| 10 | **多层配置体系（Viper 模式）** | GitHub MCP | 全局 → 项目 → 环境变量 → CLI | P1 |
| 11 | **DashMap 并发图存储** | CodePrism | ToolGraph 适配器注册和命令索引 | P1 |
| 12 | **Mandrel MCP Test Harness** | CodePrism | WindWave 的 MCP 合规测试 | P1 |
| 13 | **`thread_id` = `PlanId`** | LangGraph | 会话标识与恢复 | P1 |
| 14 | **`toolParams` 验证模式** | Unity MCP | ToolCommand 参数验证 | P1 |
| 15 | **OAuth 2.1 + PKCE** | GitHub MCP | 凭证安全存储 | P1 |
| 16 | **内容窗口管理（content-window-size）** | GitHub MCP | 上下文膨胀控制 | P1 |
| 17 | **Streamable HTTP 会话管理** | MCP Spec | 远程适配器会话恢复 | P1 |
| 18 | **`tools/list` 分页 + `listChanged` 通知** | MCP Spec | 大量命令时的分页支持 | P1 |
| 19 | **6 种内容类型（Text/Image/Audio/Resource）** | MCP Spec | `ToolResult` enum 完整支持 | P1 |
| 20 | **Chrome Extension 桥接模式** | PlayCanvas MCP | Figma 等 Web 工具的桥接 | P1 |
| 21 | **显式连接确认** | PlayCanvas MCP | 每个 Adapter 的授权流程 | P1 |
| 22 | **安全沙箱（SandboxConfig）** | CodePrism | 每个 ToolAdapter 的资源限制 | P2 |
| 23 | **多级缓存策略** | CodePrism | 命令结果缓存 | P2 |
| 24 | **Insiders 实验通道** | GitHub MCP | 实验功能开关 | P2 |
| 25 | **Lockdown 模式** | GitHub MCP | 敏感操作二次确认 | P2 |
| 26 | **WebSocket 实时通信** | PlayCanvas MCP | 实时协作场景 | P2 |
| 27 | **WASM 插件扩展** | CodePrism | 第三方适配器市场 | P3 |
| 28 | **Store 集成** | PlayCanvas MCP | 资源市场/素材库集成 | P3 |

#### 14.8.2 关键架构决策汇总（最终版）

| 决策 | 选择 | 理由 | 来源 |
|---|---|---|---|
| 适配器接口 | `ToolAdapter` trait (Send + Sync) | 统一接口，可插拔设计 | CodePrism `AnalysisTool` |
| 并发存储 | `DashMap` 而非 `RwLock<HashMap>` | 更高并发性能 | CodePrism `CodeGraph` |
| 工具过滤 | 三层过滤（配置/权限/黑名单） | 灵活可控，避免上下文膨胀 | GitHub MCP toolsets |
| 配置管理 | 多层配置（全局→项目→环境→CLI） | 灵活覆盖，生产就绪 | GitHub MCP Viper 模式 |
| 传输模式 | 本地（stdio）+ 远程（Streamable HTTP） | 覆盖所有工具类型 | MCP Spec + GitHub MCP |
| 执行引擎 | StateGraph + Checkpointer | 持久化、可恢复、可中断 | LangGraph |
| 测试策略 | Mandrel MCP Test Harness | MCP 协议合规验证 | CodePrism `moth` |
| 安全模型 | `SandboxConfig` 每个适配器 | 最小权限，错误隔离 | CodePrism 安全设计 |
| 工具注解 | `ToolAnnotations`（readOnlyHint 等） | 驱动审批流和重试策略 | MCP Spec |
| 工具结果 | 6 种内容类型 | 完整覆盖所有输出场景 | MCP Spec |
| 协议合规 | 严格遵循 MCP 2025-06-18 规范 | 与生态完全兼容 | MCP Spec |
| Web 工具桥接 | Chrome Extension + WebSocket | 覆盖 Figma 等 Web 工具 | PlayCanvas MCP |

#### 14.8.3 代码哲学教训（最终版）

来自六个项目的共同教训：

1. **避免过早抽象**（Unity MCP）— Phase 1 不要过度设计 ToolAdapter trait
2. **Domain Symmetry**（Unity MCP）— 命令与目标工具的原生语义一致
3. **删除而非弃用**（Unity MCP）— 不保留向后兼容 shim
4. **测试覆盖要求**（Unity MCP）— 每个新适配器必须有对应的测试
5. **无状态设计**（GitHub MCP）— 每次请求携带完整认证信息
6. **实验通道**（GitHub MCP）— Insiders 模式允许快速迭代
7. **并发优先**（CodePrism）— DashMap 而非 RwLock，从第一天就考虑并发
8. **AI-first 工程**（CodePrism）— 100% AI 生成的代码库证明了 AI 驱动开发的可行性
9. **持久化执行**（LangGraph）— checkpointing 是复杂工作流的基础
10. **HITL 内置**（LangGraph）— 审批流应该是执行引擎的内置能力
11. **协议优先**（MCP Spec）— 严格遵循 MCP 规范，确保生态兼容性
12. **工具注解驱动行为**（MCP Spec）— 用 `readOnlyHint`/`destructiveHint` 等元数据驱动自动化决策
13. **上下文限制是真实问题**（PlayCanvas MCP）— 工具列表必须可过滤，验证了 GitHub MCP toolsets 的必要性
14. **显式用户授权**（PlayCanvas MCP）— 每个 Adapter 需要用户显式连接确认

#### 14.8.4 实施路线图（最终版）

```
Phase 0 (当前): 基础工具适配器
  ├── Blender MCP Adapter (本地 stdio 模式)
  ├── Godot MCP Adapter (本地 stdio 模式)
  └── VS Code MCP Adapter (本地 stdio 模式)

Phase 1 (P0): 核心架构 + 协议合规
  ├── ToolAdapter trait 标准化 (借鉴 CodePrism AnalysisTool)
  ├── ToolOrchestrator + StateGraph 执行引擎 (借鉴 LangGraph)
  ├── CapabilityGroup 工具分组 (借鉴 GitHub MCP toolsets)
  ├── 三层过滤机制 (借鉴 GitHub MCP 三层过滤)
  ├── PlanId + Checkpointer 持久化 (借鉴 LangGraph)
  ├── MCP 初始化生命周期 (遵循 MCP 2025-06-18 Spec)
  ├── ToolAnnotations 注解系统 (遵循 MCP Spec)
  └── 两种错误模式 (遵循 MCP Spec)

Phase 2 (P1): 生产就绪
  ├── 双模传输 (本地 stdio + 远程 Streamable HTTP)
  ├── 多层配置体系 (Viper 模式)
  ├── DashMap 并发图存储 (借鉴 CodePrism CodeGraph)
  ├── Mandrel MCP Test Harness 集成
  ├── 内容窗口管理 (context-window-size)
  ├── OAuth 2.1 + PKCE 凭证存储
  ├── Streamable HTTP 会话管理
  ├── Chrome Extension 桥接 (借鉴 PlayCanvas MCP)
  └── 显式连接确认流程

Phase 3 (P2): 增强功能
  ├── 安全沙箱 (SandboxConfig)
  ├── 多级缓存策略
  ├── Insiders 实验通道
  ├── Lockdown 敏感操作确认
  ├── WebSocket 实时通信
  └── 命令结果缓存

Phase 4 (P3): 生态扩展
  ├── WASM 插件扩展
  ├── 第三方适配器市场
  ├── Store 资源市场集成
  └── 社区贡献的 ToolAdapter
```