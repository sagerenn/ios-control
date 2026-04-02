# Coordinate Mapping And Action Grounding Design

Date: 2026-04-02
Status: Design approved, awaiting written spec review

## Scope

This spec defines the action grounding layer that maps observed iOS UI targets into feasible HID action sequences for stock iPhone and iPad devices.

The agreed constraints are:

- The target domain is general-purpose arbitrary iPhone and iPad UI.
- The system is fully autonomous when confidence allows.
- This spec covers action grounding and recovery, not high-level goal planning.
- Inputs may be mixed:
  - Semantic target when available
  - Visual target or region when semantics are weak
- The action space is hybrid HID:
  - Pointer actions
  - Keyboard actions
  - Mode switching
  - Retries
- Recovery policy is conservative.
- The system maintains a tracked virtual pointer state with uncertainty.

## Problem Statement

The overall project needs a layer that can convert "what should be activated on the iOS screen" into "which HID actions should be sent over the Bluetooth transport."

That problem is constrained by the rest of the system:

- Capture provides only observed pixels and metadata.
- Bluetooth control provides only HID-semantic input, not direct tap injection.
- Stock iOS does not provide direct pointer or focus telemetry back to the host.

So the grounding layer cannot assume exact screen coordinates or authoritative input state. It must operate on estimates, confidence, and post-action observation.

## Goals

- Accept mixed semantic and visual target inputs.
- Choose between pointer, keyboard, and hybrid grounding strategies.
- Maintain explicit internal estimates for pointer and focus state.
- Emit only valid abstract HID actions into the Bluetooth transport.
- Verify attempted actions against observed screen changes.
- Fail conservatively when confidence is insufficient.

## Non-Goals

- High-level task planning across long workflows.
- Perfect coordinate truth.
- Open-ended autonomous exploration.
- Jailbreak-only control paths.
- Direct Bluetooth packet handling.

## Product Definition

This subsystem exposes a `GroundingEngine` between higher-level planners and the Bluetooth transport.

Its responsibility is narrow and explicit:

- Take a target
- Evaluate how reachable it is under current uncertainty
- Choose a bounded HID plan
- Observe the result
- Update internal state
- Either succeed or fail fast

It does not decide the long-term goal, invent new tasks, or explore the UI indefinitely.

## Architecture

The subsystem has four core parts:

1. Target Resolver
2. Interaction State
3. Action Selector
4. Recovery Controller

### 1. Target Resolver

This component accepts mixed target input:

- Semantic target such as "activate Settings button"
- Visual target such as a region or coordinate

It resolves that input into one or more candidate targets with confidence and likely interaction affordances.

### 2. Interaction State

This component maintains two internal state models:

- `virtual pointer state`
- `focus state`

The virtual pointer state includes:

- Estimated pointer position
- Uncertainty radius
- Screen transform
- Calibration freshness

The focus state includes:

- Estimated focus location
- Recent navigation history
- Focus confidence

### 3. Action Selector

This component scores candidate grounding strategies:

- Pointer path
- Keyboard path
- Mixed path

It selects the lowest-cost feasible plan based on confidence, distance, expected step count, and recovery risk.

### 4. Recovery Controller

This component enforces the conservative recovery policy.

It may:

- Perform one obvious retry
- Apply one simple recovery action when clearly justified
- Return failure immediately when confidence collapses

It does not perform aggressive exploration.

## Components

### Target Resolver

Inputs:

- Semantic label or intent
- Visual region or coordinate
- Target confidence
- Optional type hints such as `button`, `text field`, `scroll area`, or `tab bar item`

Outputs:

- Candidate targets
- Candidate action type
- Likely affordances:
  - Pointer-friendly
  - Keyboard-focusable
  - Ambiguous

This component avoids treating every target as equally actionable.

### Coordinate Mapper

This is the geometry core.

It maps between:

- Capture-space coordinates
- Normalized device-space coordinates
- Virtual pointer-space estimates

Tracked state includes:

- Capture size and orientation
- Crop or letterbox offsets when present
- Estimated pointer position
- Uncertainty radius
- Last recalibration evidence and time

The mapper maintains an estimate, not ground truth.

### Focus Tracker

This component models keyboard navigation state under Full Keyboard Access.

Tracked state includes:

- Last known focus candidate
- Likely next and previous focus ordering
- Whether focus indicators are visually detectable
- Confidence that the current screen is keyboard-friendly

### Action Selector

This decision component evaluates:

- Pointer-only plans
- Keyboard-only plans
- Hybrid plans

Scoring factors include:

- Target confidence
- Pointer uncertainty
- Estimated step count
- Need for text entry
- Expected recoverability

The chosen plan must remain inside the conservative recovery budget.

### Execution Monitor

This component watches post-action frames and checks for expected evidence:

- Focus moved
- Screen changed
- Target disappeared because it was activated
- Keyboard appeared
- Scroll position changed

It decides whether the attempted action:

- Succeeded
- Justifies one obvious retry
- Failed

### Recovery Controller

This component owns the bounded recovery set.

Allowed behavior:

- Retry once when failure is obvious
- Issue one simple reset-style action if clearly justified
- Return structured failure when confidence is low

Disallowed behavior:

- Indefinite probing
- Long exploratory search loops

## Data Flow

The grounding layer receives a target from above and returns a bounded HID plan plus an execution result.

### 1. Input

Higher layers provide:

- Semantic target, visual target, or both
- Current frame metadata from the capture subsystem
- Optional scene annotations
- Execution context such as whether text entry or scrolling is expected

### 2. Resolve

The Target Resolver produces one or more actionable candidates, each with:

- Target region
- Likely action type
- Interaction affordances
- Confidence

### 3. State Alignment

The Coordinate Mapper and Focus Tracker align the candidates against current internal state:

- Map target into current device and capture geometry
- Estimate pointer path length and uncertainty
- Estimate keyboard reachability and likely step count

### 4. Plan Selection

The Action Selector scores:

- Pointer plan
- Keyboard plan
- Hybrid plan

It chooses the lowest-cost plan that stays within the recovery budget.

Typical outputs:

- `move pointer -> click`
- `Tab x3 -> Enter`
- `small pointer move -> click -> keyboard text entry`

### 5. Execute

The chosen plan is emitted as abstract HID actions into the Bluetooth transport layer.

Representative actions:

- `PointerMove(dx, dy)`
- `PointerClick(button)`
- `PointerScroll(dx, dy)`
- `KeyPress(code, modifiers)`
- `TextEntry(text)`
- `ModeSwitch(pointer|keyboard|hybrid)` when needed internally

### 6. Observe

After each step or short step group, the Execution Monitor checks the next frames for expected evidence:

- Target activated
- Focus moved
- Text field engaged
- Screen changed
- Scroll occurred

### 7. Recover Or Fail

If evidence matches, the subsystem updates state and continues or completes.

If evidence does not match:

- Perform one obvious retry if justified
- Otherwise return structured failure with cause and confidence drop

## Error Handling

The core risk is false certainty. The subsystem must surface uncertainty rather than hide it.

Failure classes for V1:

- `TargetAmbiguous`
- `GeometryUncertain`
- `FocusUncertain`
- `ExecutionMismatch`
- `RecoveryExhausted`

### TargetAmbiguous

Multiple plausible targets match the request and the resolver cannot distinguish them safely.

Response:

- Return failure
- Preserve candidate ranking and confidence diagnostics

### GeometryUncertain

The coordinate mapper cannot justify the current transform or pointer estimate tightly enough for a pointer plan.

Response:

- Down-rank or reject pointer plans
- Prefer keyboard plans when possible
- Return failure when no justified plan remains

### FocusUncertain

The focus tracker cannot infer a stable keyboard path with enough confidence.

Response:

- Down-rank or reject keyboard plans
- Prefer pointer plans when possible
- Return failure when no justified plan remains

### ExecutionMismatch

The transport delivered the action, but the observed frames do not show the expected UI effect.

Response:

- Distinguish transport success from grounding success
- Allow one obvious retry if justified
- Otherwise fail

### RecoveryExhausted

The allowed retry budget is spent or no justified retry exists.

Response:

- Return structured failure immediately
- Avoid continued exploration

## Logging And Diagnostics

The subsystem should log:

- Input target form and confidence
- Candidate target ranking
- Pointer estimate and uncertainty radius
- Focus estimate and confidence
- Chosen plan and its score
- Post-action observations
- Retry decisions
- Structured failure causes

Logs must make it possible to explain why the system:

- Chose pointer over keyboard
- Chose keyboard over pointer
- Declared success
- Failed conservatively

## Testing Strategy

### Unit Tests

Cover shared grounding behavior:

- Coordinate transforms
- Pointer uncertainty updates
- Focus-step estimation
- Plan scoring
- Conservative retry decisions

### Simulation Tests

Use mocked frames and state transitions to verify:

- Success detection
- Fail-fast behavior
- Correct state updates after observed UI change

### Contract Tests

Verify the grounding layer emits only valid abstract HID actions into the Bluetooth transport contract.

### Acceptance Criteria

V1 is successful when:

- The subsystem can accept mixed semantic and visual targets.
- It can choose between pointer, keyboard, and hybrid plans.
- It can keep pointer uncertainty and focus confidence explicit.
- It can verify likely success from observed frames.
- It fails rather than exploring indefinitely when confidence is insufficient.

## Risks

- Relative pointer control without direct telemetry makes calibration drift unavoidable.
- Focus behavior varies across apps and screens, so keyboard grounding cannot be assumed globally reliable.
- General-purpose arbitrary UI increases ambiguity compared with app-specific automation.
- A fully autonomous top layer can still fail frequently if grounding confidence is overestimated.

## Relationship To Other Specs

This subsystem depends on:

- Screen capture for frames and frame metadata
- Bluetooth control for abstract HID action delivery

This subsystem does not replace:

- Scene understanding
- High-level task planning

It is the bounded middle layer that turns actionable targets into feasible HID sequences under uncertainty.

## References

- Apple Support: Use a pointer device with iPhone, iPad, or iPod touch
  https://support.apple.com/en-us/111775
- Apple User Guide: Control iPhone with an external keyboard
  https://support.apple.com/guide/iphone/control-iphone-with-an-external-keyboard-ipha4375873f/ios
- Apple User Guide: Use an external keyboard to control iPad
  https://support.apple.com/en-gw/guide/ipad/ipad5f765d6f/ipados
