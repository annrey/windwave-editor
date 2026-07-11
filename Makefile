SHELL := /bin/bash

.PHONY: help run check test clippy gate smoke-p0-p1 accept-open-world-qa test-ai-resident test-red-enemy test-hr test-undo test-sceneindex test-revision open-xcode

help:
	@echo "WindWave development commands"
	@echo ""
	@echo "  make run                 Run the editor"
	@echo "  make accept-open-world-qa Main-window framebuffer QA accept mode"
	@echo "  make test-ai-resident    AiResidentSlice01 core + Bevy smoke"
	@echo "  make check               cargo check"
	@echo "  make test                cargo test --workspace"
	@echo "  make clippy              cargo clippy --workspace -- -D warnings"
	@echo "  make gate                check + test + clippy"
	@echo "  make smoke-p0-p1         Focused P0/P1 regression suite"
	@echo "  make open-xcode          Open this folder in Xcode as a source browser"

run:
	cargo run --bin agent-edit

# Boots the real Bevy window, waits for ScreenshotQueue framebuffer readback,
# writes docs/qa/open-world-slice01*.{md,json,png}, exits 0 only on bevy_framebuffer.
accept-open-world-qa:
	WINDWAVE_OPEN_WORLD_QA_ACCEPT=1 cargo run --bin agent-edit

# AiResidentSlice01: schedule/permission harness + Bevy resident smoke
test-ai-resident:
	cargo test -p agent-core ai_resident
	cargo test -p bevy-adapter ai_resident

check:
	cargo check

test:
	cargo test --workspace

clippy:
	cargo clippy --workspace -- -D warnings

gate: check test clippy

smoke-p0-p1: test-red-enemy test-hr test-undo test-sceneindex test-revision test-ai-resident

test-red-enemy:
	cargo test -p bevy-adapter test_director_red_enemy_request_mutates_bevy_world_and_undoes

test-hr:
	cargo test -p agent-edit ui_smoke_hr_request

test-undo:
	cargo test -p bevy-adapter reverse
	cargo test -p bevy-adapter test_multi_undo_chain

test-sceneindex:
	cargo test -p bevy-adapter test_incremental_plugin_reconciles_deleted_entities_without_waiting_for_fallback

test-revision:
	cargo test -p agent-core test_failed_internal_plan_emits_revision_review

open-xcode:
	open -a Xcode .
