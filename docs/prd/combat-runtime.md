# Combat Runtime

> 日期：2026-06-07  
> 状态：OWS-6 初版已落地到 `crates/agent-core/src/open_world_runtime.rs`  
> 关联 PRD：`docs/prd/open-world-vertical-slice-prd.md`

## 用途

`CombatRuntime` 当前以内联类型的方式落在 `OpenWorldRuntimeState` 中，用来把 `Combatant` / `Attack` / `EnemyBrain` 从“脚本直接置死”推进到可验证的最小战斗状态机。

当前目标不是完整动作战斗，而是让对话生成的游戏切片至少能证明：

```text
player 进入战斗
  -> camp_enemy_01 从 Patrol 进入 Aggro
  -> player 多次攻击扣血
  -> camp_enemy_01 血量归零
  -> enemy state 变为 Dead
  -> DefeatEnemy objective 完成
```

## Rust API

公开模块：

```rust
agent_core::open_world_runtime
```

主要类型：

- `CombatantRuntime`
- `CombatFaction`
- `CombatHitReport`
- `EnemyRuntimeState`
- `OpenWorldRuntimeState`

主路径调用：

```rust
let plan = OpenWorldPlan::open_world_slice01_fixture();
let mut runtime = OpenWorldRuntimeState::from_plan(&plan);

runtime.engage_enemy("player", "camp_enemy_01")?;
let first_hit = runtime.attack_enemy("player", "camp_enemy_01")?;
let remaining_hits = runtime.attack_until_defeated("player", "camp_enemy_01")?;
```

## 默认战斗参数

第一版参数固定在 core fixture 中，后续可从 `OpenWorldPlan` 或 prefab/component schema 注入：

| 对象 | 阵营 | HP | Damage |
|---|---|---:|---:|
| `player` | `Player` | 100 | 10 |
| `camp_enemy_01` | `Enemy` | 30 | 6 |

因此 `player` 需要 3 次命中才能击败 `camp_enemy_01`。

## 状态规则

### 敌人状态

```text
Patrol
  -> Aggro
  -> Attacking
  -> Dead
```

- `from_plan()` 会把敌人初始化为 `Patrol`。
- `engage_enemy()` 要求玩家和敌人都有 `Combatant`，敌人有 `EnemyBrain`，成功后进入 `Aggro`。
- `attack_enemy()` 要求玩家有 `Attack + Combatant`，敌人有 `Combatant + EnemyBrain`。
- 敌人必须处于 `Aggro` 或 `Attacking` 才能被攻击。
- 每次攻击都会生成 `CombatHitReport` 和 `OpenWorldRuntimeEvent::CombatHit`。
- HP 归零时敌人进入 `Dead`，`defeat_camp_enemy` objective 进入 `Completed`。

### 失败报告

当前已覆盖：

- 未 engage 就攻击：提示先 `engage camp_enemy_01 before attacking`。
- 玩家缺少 `Attack`：提示 `add Attack to player`。
- 敌人缺少 `Combatant`：旧 `PlayableScenario` 失败报告仍可定位到 `camp_enemy_01`。

## 当前测试

窄测试：

```bash
cargo test -p agent-core open_world_runtime
```

已覆盖：

- 攻击会扣血，未死亡前 objective 不完成。
- `attack_until_defeated()` 会通过多次攻击把敌人推进到 `Dead`。
- 敌人死亡后 `DefeatEnemy` objective 完成，主任务推进到 `EnemyDefeated`。
- 未进入 `Aggro` 不能攻击。
- 缺少 `Attack` 不能造成伤害。

回归测试：

```bash
cargo test -p agent-core playable_scenario
```

已确认 `OpenWorldSlice01` 主线 playtest 仍通过。

## 后续接线

1. 将 `CombatantRuntime` 参数从固定 fixture 改为 plan/component schema 输入。
2. 将 hit/dead event 映射到 SceneIndex 查询状态。
3. 将 `Attack` 接入 Bevy ECS 或 scripted input runner。
4. 增加玩家受击、失败/重置规则。
