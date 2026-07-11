# QA: AiResidentSlice01

> 日期：2026-07-11  
> 状态：**自动 playtest Passed**（core harness）  
> PRD：`docs/prd/ai-resident-sandbox.md`  
> 前置：OpenWorldSlice01 主窗口 framebuffer 已 Passed（`make accept-open-world-qa`）

## 目标

证明少量受限居民（`merchant_01` + `guard_01`）在 OpenWorldSlice01 小岛上：

1. 营业窗口内允许 `quote_price`，窗外拒绝并进证据  
2. 巡逻窗口内允许 `patrol_waypoint`，窗外拒绝 / `fallback_behavior` 可观测  
3. 越权动作被拒并写入 `ReplayLedger` / VerificationBundle  
4. SceneIndex 风格观察含 `agent_id` / 角色 / 日程 / 区域  
5. 规模上限 ≤20（超出配置失败）

## 自动验收（必跑）

```bash
# 推荐：固定 target dir，避免 iCloud 路径下编译慢
export CARGO_TARGET_DIR=/private/tmp/windwave-target

# Core harness（PRD §5 成功标准 1–5、7）
cargo test -p agent-core ai_resident -- --nocapture

# Bevy 行为桥接（营业态 / 巡逻组件 + SceneIndex 摘要）
cargo test -p bevy-adapter ai_resident -- --nocapture

# 或 Makefile 聚合
make test-ai-resident
```

期望：全部 Passed；core playtest 产出的 VerificationBundle Markdown 含 `Schedule Decisions` 与 deny 类 `Agent Events`。

## OpenWorld 回归（沙盘不得破坏）

```bash
# 主线自动 playtest（verification bundle）
cargo test -p agent-core verification_bundle_passes_open_world_slice01_main_path

# 主窗口 framebuffer（真人/accept 门禁，沙盘不要求新开）
make accept-open-world-qa
```

## 可选手动点验（GUI）

沙盘第一刀**不**要求新开主窗口门禁。若要在编辑器里肉眼看居民：

1. 启动 `make run`  
2. 加载 OpenWorldSlice01  
3. 若已挂 `AiResidentPlugin` 并 spawn 居民：冻结时钟到 12:00 看商人 `shop_open`；到 23:00 应变为 idle；营地侧 guard 在巡逻窗口应在两 waypoint 间移动  

当前自动证据以 core + headless Bevy smoke 为准。

## 证据字段

| 字段 | 来源 |
|---|---|
| `schedule_decisions` | `AgentSchedule` @ `WorldClock` |
| `agent_events` | 裁决 allow/deny/fallback 行 |
| `scene_index_observations` | `agent_id=… role=… zone=… schedule=…` |
| `ReplayLedger` | `resident_allow` / `resident_deny` / `resident_fallback` |

## 明确不做（本刀）

工具中枢、真多人、完整 Governance Desk、LLM 自由写世界、Quest/Explorer 全量角色。
