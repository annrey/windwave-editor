# GameplayPrimitive Catalog

> 日期：2026-06-07  
> 状态：OWS-1 初版已落地到 `crates/agent-core/src/gameplay_primitive.rs`  
> 关联 PRD：`docs/prd/open-world-vertical-slice-prd.md`

## 用途

`GameplayPrimitive` 是 AI 生成可玩垂直切片时允许引用的玩法积木。AI 计划只能引用 catalog 中已登记的 primitive；如果需要未登记能力，必须报告缺口，而不是假装能执行。

第一版 catalog 只定义能力边界，不实现 Bevy 组件行为。运行时组件映射和具体 gameplay 系统在后续 OWS-4/OWS-5/OWS-6 中实现。

## Rust API

公开模块：

```rust
agent_core::gameplay_primitive
```

主要类型：

- `GameplayPrimitiveKind`
- `GameplayPrimitiveCategory`
- `GameplayCapability`
- `GameplayPrimitiveDescriptor`
- `GameplayPrimitiveCatalog`
- `GameplayPrimitiveValidationError`

默认 catalog：

```rust
let catalog = GameplayPrimitiveCatalog::open_world_slice_defaults();
```

校验计划引用：

```rust
catalog.validate_references(&[
    GameplayPrimitiveKind::PlayerController,
    GameplayPrimitiveKind::Quest,
    GameplayPrimitiveKind::Combatant,
])?;
```

## 第一版 primitive 清单

| Primitive | Category | 最低能力 | 依赖 |
|---|---|---|---|
| `PlayerController` | Player | movement, facing, speed limit | `WorldSurface` |
| `FollowCamera` | Camera | target follow, offset, stable update | `PlayerController` |
| `Interactable` | Interaction | enter range, trigger interaction, emit event | `InteractionZone` |
| `InteractionZone` | Interaction | range check, target binding | - |
| `Quest` | Quest | state, objective list, completion check | `QuestObjective` |
| `QuestObjective` | Quest | objective kind, target id, completion state | - |
| `PuzzleSwitch` | Puzzle | locked, awaiting interaction, solved | `Interactable` |
| `LootContainer` | Loot | locked, unlocked, opened, loot claimed | `Inventory` |
| `Inventory` | Loot | add item, contains item, prevent duplicates | - |
| `Combatant` | Combat | health, faction, defeated state | - |
| `Attack` | Combat | damage, cooldown, target | `Combatant` |
| `EnemyBrain` | Combat | idle, patrol, aggro, attack, dead | `Combatant`, `ZoneMarker` |
| `WorldSurface` | World | walkable area, queryable bounds | - |
| `ZoneMarker` | World | stable id, bounds, semantic label | - |

## 当前测试

窄测试：

```bash
cargo test -p agent-core gameplay_primitive
```

已覆盖：

- 默认 catalog 包含 `OpenWorldSlice01` 所需 primitives。
- `EnemyBrain` 等关键依赖可查询。
- 空 catalog 会拒绝缺失 primitive。
- catalog 可 JSON round-trip。

## 后续接线

下一步不应直接写大量 gameplay 系统，而应先做：

1. `OpenWorldPlan` schema 引用 `GameplayPrimitiveKind`。
2. `OpenWorldPlan` fixture 通过 `GameplayPrimitiveCatalog::validate_references()`。
3. `PlayableScenario` 失败报告能指出缺失 primitive 或缺失组件。
4. Bevy 组件映射在 OWS-4/OWS-5/OWS-6 按场景、任务、战斗分别实现。
