# QA: Red Enemy Closed-Loop Scenario

> Updated: 2026-06-07  
> Purpose: define the v0.2.0 P0 acceptance path for WindWave closed-loop execution.

## Scenario

User request:

```text
创建一个红色敌人放在右边
```

Expected loop:

```text
user request -> plan -> act -> observe -> revise -> verify -> undo
```

## Automated Check

There is an existing agent-core acceptance test:

```bash
cargo test -p agent-core test_acceptance_create_red_enemy_scenario -- --nocapture
```

Current limitation: this test verifies that the DirectorRuntime path returns a response and emits trace/event information. It does not yet prove that a real Bevy World changed, that SceneIndex observed the new entity, or that undo removed it. Those are the remaining v0.2.0 closed-loop gaps.

The stronger Bevy-side closed-loop regression test has been added:

```bash
cargo test -p bevy-adapter test_director_red_enemy_request_mutates_bevy_world_and_undoes
```

This test covers `DirectorRuntime -> SceneBridge -> EngineCommand -> Bevy World -> SceneIndex -> undo`. Status on 2026-06-07: passed in the original iCloud Drive checkout.

SceneIndex deletion cleanup has a focused regression test:

```bash
cargo test -p bevy-adapter test_incremental_plugin_reconciles_deleted_entities_without_waiting_for_fallback
```

This test verifies that a despawned entity is removed from `SceneIndexCache` without waiting for the fallback rebuild interval.

Failure revision has a focused regression test:

```bash
cargo test -p agent-core test_failed_internal_plan_emits_revision_review
```

This test verifies that an internal execution failure emits `ReviewCompleted(needs_revision)` and rewrites the failed plan step with a DynamicPlanner revision or ReflectionEngine alternative. Status on 2026-06-07: passed in the original iCloud Drive checkout.

Undo/redo reverse contracts have focused regression tests:

```bash
cargo test -p bevy-adapter test_spawn_prefab_reverse_is_delete
cargo test -p bevy-adapter test_set_sprite_texture_reverse_removes_added_sprite
cargo test -p bevy-adapter test_multi_undo_chain
```

`LoadAsset` is intentionally treated as idempotent and low-risk for now, so it does not generate a reverse command. If asset hot-unload becomes required, introduce a dedicated `RemoveAssetReference` command instead of pretending `LoadAsset` can be faithfully undone.

HR approval UI actions have an automated smoke test:

```bash
cargo test -p agent-edit ui_smoke_hr_request
```

Status on 2026-06-07: passed. It covers `hire agent` routing through the main-app systems, Director Desk pending approval sync, approve/reject actions, desk clearing, Director pending clearing, and roster mutation/non-mutation.

## Manual QA Path

1. Start the editor:

   ```bash
   cargo run --bin agent-edit
   ```

   2026-06-07 runtime note: the editor starts in the iCloud checkout. The Bevy PNG feature is now enabled, so the previous repeated screenshot error `Cannot save screenshot, IO error: The image format Png is not supported` no longer appears. Computer Use still does not list the Bevy/winit window as a controllable macOS app, so Codex cannot currently perform the final click-through manually.

2. In the chat input, enter:

   ```text
   创建一个红色敌人放在右边
   ```

3. Observe the Director Desk:

   - A plan or direct execution trace should appear.
   - Any high-risk operation should enter approval before applying.
   - The execution trace should include create/color/position intent.

4. Observe the scene:

   - A new enemy-like entity should appear.
   - It should be visually red, or the command trace should include `SetSpriteColor` with red RGBA.
   - It should be positioned to the right of the origin or the intended reference entity.

5. Observe SceneIndex / hierarchy:

   - The new entity should be listed.
   - Its transform should reflect the right-side placement.
   - Its component summary should reflect sprite/color data where available.

6. Test undo:

   - Trigger Undo from Director Desk or the app undo action.
   - The created entity should disappear or the scene should return to the prior state.
   - The undo action should create a redo entry.

## Pass Criteria

- The request produces a real Bevy scene mutation.
- SceneIndex observes the mutation after command processing.
- UI, Director events, and SceneIndex agree on the same operation.
- Undo reverses the mutation.
- No panic occurs.

## Known Gaps To Close

- The Bevy-side closed-loop test passes in the original checkout.
- Internal failure/revision behavior emits a revision review event and passes its focused test in the original checkout; a later real ReAct failure smoke is still useful.
- SceneIndex deleted-entity cleanup passes its focused regression test.
- Prefab and sprite texture reverse contracts have focused tests; full window-level UI undo/redo still needs human confirmation because Computer Use does not expose the Bevy/winit window.
- Reverse commands for asset/prefab/sprite texture operations must stay explicit.
