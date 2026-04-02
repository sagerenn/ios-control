# Coordinate Grounding Engine Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the action-grounding subsystem that chooses pointer, keyboard, or hybrid HID plans from mixed semantic and visual targets while keeping uncertainty and conservative recovery explicit.

**Architecture:** Implement the grounding engine as a local plugin so it can be swapped or disabled per session. Put shared target and result types in the contracts crate, then implement the core engine in `plugins/grounding-core` with focused modules for coordinate mapping, focus tracking, action selection, execution monitoring, and recovery.

**Tech Stack:** Rust, tokio, serde, thiserror, tracing

---

**Prerequisite:** Execute [2026-04-02-host-app-foundation-linux-windows.md](/home/ubuntu/ios-control/docs/superpowers/plans/2026-04-02-host-app-foundation-linux-windows.md) first. This plan also expects the capture and control plans to have established `capture.rs` and `control.rs` contract modules.

## File Structure

- `crates/contracts/src/grounding.rs`: target input, plan output, and failure types.
- `plugins/grounding-core/src/main.rs`: grounding plugin entrypoint.
- `plugins/grounding-core/src/lib.rs`: grounding plugin exports used by tests.
- `plugins/grounding-core/src/target_resolver.rs`: mixed semantic/visual target resolution.
- `plugins/grounding-core/src/coordinate_mapper.rs`: screen transform and virtual pointer estimation.
- `plugins/grounding-core/src/focus_tracker.rs`: keyboard focus inference.
- `plugins/grounding-core/src/action_selector.rs`: pointer/keyboard/hybrid scoring.
- `plugins/grounding-core/src/execution_monitor.rs`: post-action observation checks.
- `plugins/grounding-core/src/recovery_controller.rs`: conservative retry policy.
- `apps/host-desktop/src/panels/diagnostics.rs`: grounding confidence and failure display.

### Task 1: Add Grounding Contracts And Plugin Inputs

**Files:**
- Modify: `Cargo.toml`
- Modify: `crates/contracts/src/lib.rs`
- Create: `crates/contracts/src/grounding.rs`
- Test: `crates/contracts/tests/grounding_contract.rs`

- [ ] **Step 1: Write the failing test**

```rust
use ios_control_contracts::grounding::{GroundingFailure, PlanKind, TargetInput};

#[test]
fn target_input_accepts_semantic_and_visual_data() {
    let target = TargetInput {
        semantic_label: Some("Settings".into()),
        visual_region: Some((10, 20, 30, 40)),
        confidence: 0.85,
    };

    assert_eq!(target.semantic_label.as_deref(), Some("Settings"));
    assert_eq!(target.visual_region, Some((10, 20, 30, 40)));
    assert_eq!(PlanKind::Hybrid.as_str(), "hybrid");
    assert_eq!(GroundingFailure::RecoveryExhausted.as_str(), "recovery_exhausted");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p ios-control-contracts target_input_accepts_semantic_and_visual_data -- --exact`
Expected: FAIL with `could not find 'grounding' in 'ios_control_contracts'`

- [ ] **Step 3: Write minimal implementation**

```rust
// crates/contracts/src/lib.rs
pub mod capture;
pub mod control;
pub mod grounding;
pub mod plugin;
pub mod session;
```

```rust
// crates/contracts/src/grounding.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TargetInput {
    pub semantic_label: Option<String>,
    pub visual_region: Option<(u32, u32, u32, u32)>,
    pub confidence: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanKind {
    Pointer,
    Keyboard,
    Hybrid,
}

impl PlanKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pointer => "pointer",
            Self::Keyboard => "keyboard",
            Self::Hybrid => "hybrid",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GroundingFailure {
    TargetAmbiguous,
    GeometryUncertain,
    FocusUncertain,
    ExecutionMismatch,
    RecoveryExhausted,
}

impl GroundingFailure {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TargetAmbiguous => "target_ambiguous",
            Self::GeometryUncertain => "geometry_uncertain",
            Self::FocusUncertain => "focus_uncertain",
            Self::ExecutionMismatch => "execution_mismatch",
            Self::RecoveryExhausted => "recovery_exhausted",
        }
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p ios-control-contracts target_input_accepts_semantic_and_visual_data -- --exact`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/contracts/src/lib.rs crates/contracts/src/grounding.rs crates/contracts/tests/grounding_contract.rs
git commit -m "feat: add grounding contracts"
```

### Task 2: Implement Coordinate Mapper And Focus Tracker

**Files:**
- Modify: `Cargo.toml`
- Create: `plugins/grounding-core/Cargo.toml`
- Create: `plugins/grounding-core/src/lib.rs`
- Create: `plugins/grounding-core/src/main.rs`
- Create: `plugins/grounding-core/src/target_resolver.rs`
- Create: `plugins/grounding-core/src/coordinate_mapper.rs`
- Create: `plugins/grounding-core/src/focus_tracker.rs`
- Test: `plugins/grounding-core/tests/geometry.rs`

- [ ] **Step 1: Write the failing test**

```rust
use plugin_grounding_core::coordinate_mapper::CoordinateMapper;

#[test]
fn pointer_plan_is_rejected_when_uncertainty_exceeds_target_size() {
    let mapper = CoordinateMapper::new((1179, 2556), (400.0, 400.0), 120.0);

    assert!(!mapper.can_confidently_hit((350, 350, 40, 40)));
    assert!(mapper.can_confidently_hit((350, 350, 320, 320)));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p plugin-grounding-core pointer_plan_is_rejected_when_uncertainty_exceeds_target_size -- --exact`
Expected: FAIL with `package ID specification 'plugin-grounding-core' did not match any packages`

- [ ] **Step 3: Write minimal implementation**

```rust
// plugins/grounding-core/src/coordinate_mapper.rs
pub struct CoordinateMapper {
    device_size: (u32, u32),
    pointer_estimate: (f32, f32),
    uncertainty_radius: f32,
}

impl CoordinateMapper {
    pub fn new(device_size: (u32, u32), pointer_estimate: (f32, f32), uncertainty_radius: f32) -> Self {
        Self {
            device_size,
            pointer_estimate,
            uncertainty_radius,
        }
    }

    pub fn can_confidently_hit(&self, region: (u32, u32, u32, u32)) -> bool {
        let (_, _, width, height) = region;
        (width as f32) > self.uncertainty_radius * 2.0 && (height as f32) > self.uncertainty_radius * 2.0
    }

    pub fn device_size(&self) -> (u32, u32) {
        self.device_size
    }
}
```

```toml
# plugins/grounding-core/Cargo.toml
[package]
name = "plugin-grounding-core"
version = "0.1.0"
edition = "2021"

[dependencies]
ios-control-contracts = { path = "../../crates/contracts" }
```

```toml
# Cargo.toml
[workspace]
members = [
  "crates/contracts",
  "crates/plugin-protocol",
  "crates/plugin-runtime",
  "crates/capability-registry",
  "crates/device-registry",
  "crates/session-orchestrator",
  "crates/telemetry-store",
  "apps/host-desktop",
  "plugins/grounding-core",
  "plugins/mock-device",
]
resolver = "2"
```

```rust
// plugins/grounding-core/src/lib.rs
pub mod coordinate_mapper;
pub mod focus_tracker;
pub mod target_resolver;
```

```rust
// plugins/grounding-core/src/target_resolver.rs
use ios_control_contracts::grounding::TargetInput;

pub fn prefers_semantic_target(input: &TargetInput) -> bool {
    input.semantic_label.is_some()
}
```

```rust
// plugins/grounding-core/src/focus_tracker.rs
#[derive(Debug, Clone, Default)]
pub struct FocusTracker {
    pub focus_confidence: f32,
    pub keyboard_friendly: bool,
}

impl FocusTracker {
    pub fn prefers_keyboard(&self) -> bool {
        self.keyboard_friendly && self.focus_confidence >= 0.7
    }
}
```

```rust
// plugins/grounding-core/src/main.rs
fn main() {
    println!("plugin-grounding-core");
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p plugin-grounding-core pointer_plan_is_rejected_when_uncertainty_exceeds_target_size -- --exact`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml plugins/grounding-core/Cargo.toml plugins/grounding-core/src/main.rs plugins/grounding-core/src/coordinate_mapper.rs plugins/grounding-core/src/focus_tracker.rs plugins/grounding-core/tests/geometry.rs
git commit -m "feat: add grounding state models"
```

### Task 3: Implement Action Selection, Execution Monitoring, And Recovery

**Files:**
- Modify: `plugins/grounding-core/src/lib.rs`
- Create: `plugins/grounding-core/src/action_selector.rs`
- Create: `plugins/grounding-core/src/execution_monitor.rs`
- Create: `plugins/grounding-core/src/recovery_controller.rs`
- Test: `plugins/grounding-core/tests/plans.rs`

- [ ] **Step 1: Write the failing test**

```rust
use ios_control_contracts::grounding::PlanKind;
use plugin_grounding_core::action_selector::ActionSelector;
use plugin_grounding_core::focus_tracker::FocusTracker;

#[test]
fn keyboard_plan_wins_when_focus_confidence_is_high() {
    let selector = ActionSelector::default();
    let focus = FocusTracker {
        focus_confidence: 0.9,
        keyboard_friendly: true,
    };

    let plan = selector.choose_plan(true, &focus, 120.0).unwrap();

    assert_eq!(plan.kind, PlanKind::Keyboard);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p plugin-grounding-core keyboard_plan_wins_when_focus_confidence_is_high -- --exact`
Expected: FAIL with `could not find 'action_selector' in 'plugin_grounding_core'`

- [ ] **Step 3: Write minimal implementation**

```rust
// plugins/grounding-core/src/action_selector.rs
use ios_control_contracts::grounding::{GroundingFailure, PlanKind};

use crate::focus_tracker::FocusTracker;

#[derive(Debug, Clone, PartialEq)]
pub struct SelectedPlan {
    pub kind: PlanKind,
}

#[derive(Debug, Default)]
pub struct ActionSelector;

impl ActionSelector {
    pub fn choose_plan(
        &self,
        pointer_possible: bool,
        focus: &FocusTracker,
        pointer_uncertainty: f32,
    ) -> Result<SelectedPlan, GroundingFailure> {
        if focus.prefers_keyboard() {
            return Ok(SelectedPlan { kind: PlanKind::Keyboard });
        }

        if pointer_possible && pointer_uncertainty < 80.0 {
            return Ok(SelectedPlan { kind: PlanKind::Pointer });
        }

        Err(GroundingFailure::GeometryUncertain)
    }
}
```

```rust
// plugins/grounding-core/src/execution_monitor.rs
pub struct ExecutionMonitor;

impl ExecutionMonitor {
    pub fn screen_changed(before: u64, after: u64) -> bool {
        before != after
    }
}
```

```rust
// plugins/grounding-core/src/recovery_controller.rs
use ios_control_contracts::grounding::GroundingFailure;

#[derive(Debug, Default)]
pub struct RecoveryController {
    retries_used: u8,
}

impl RecoveryController {
    pub fn next_action(&mut self, obvious_retry: bool) -> Result<bool, GroundingFailure> {
        if obvious_retry && self.retries_used == 0 {
            self.retries_used += 1;
            return Ok(true);
        }

        Err(GroundingFailure::RecoveryExhausted)
    }
}
```

```rust
// plugins/grounding-core/src/lib.rs
pub mod action_selector;
pub mod coordinate_mapper;
pub mod execution_monitor;
pub mod focus_tracker;
pub mod recovery_controller;
pub mod target_resolver;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p plugin-grounding-core keyboard_plan_wins_when_focus_confidence_is_high -- --exact`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add plugins/grounding-core/src/lib.rs plugins/grounding-core/src/action_selector.rs plugins/grounding-core/src/execution_monitor.rs plugins/grounding-core/src/recovery_controller.rs plugins/grounding-core/tests/plans.rs
git commit -m "feat: add grounding plan selection"
```

### Task 4: Add Grounding Diagnostics To The Desktop Shell

**Files:**
- Modify: `apps/host-desktop/src/panels/diagnostics.rs`
- Test: `apps/host-desktop/tests/grounding_diagnostics.rs`

- [ ] **Step 1: Write the failing test**

```rust
use host_desktop::panels::diagnostics::GroundingDiagnosticsViewModel;

#[test]
fn grounding_diagnostics_formats_uncertainty_and_failure() {
    let view_model = GroundingDiagnosticsViewModel {
        pointer_uncertainty: 96.0,
        focus_confidence: 0.32,
        last_failure: Some("geometry_uncertain".into()),
    };

    assert!(view_model.summary().contains("96.0"));
    assert!(view_model.summary().contains("geometry_uncertain"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p host-desktop grounding_diagnostics_formats_uncertainty_and_failure -- --exact`
Expected: FAIL with `no struct named 'GroundingDiagnosticsViewModel' found`

- [ ] **Step 3: Write minimal implementation**

```rust
// apps/host-desktop/src/panels/diagnostics.rs
#[derive(Debug, Clone, PartialEq)]
pub struct GroundingDiagnosticsViewModel {
    pub pointer_uncertainty: f32,
    pub focus_confidence: f32,
    pub last_failure: Option<String>,
}

impl GroundingDiagnosticsViewModel {
    pub fn summary(&self) -> String {
        format!(
            "pointer uncertainty {:.1}, focus {:.2}, last failure {}",
            self.pointer_uncertainty,
            self.focus_confidence,
            self.last_failure.clone().unwrap_or_else(|| "none".into())
        )
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p host-desktop grounding_diagnostics_formats_uncertainty_and_failure -- --exact`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add apps/host-desktop/src/panels/diagnostics.rs apps/host-desktop/tests/grounding_diagnostics.rs
git commit -m "feat: add grounding diagnostics"
```
