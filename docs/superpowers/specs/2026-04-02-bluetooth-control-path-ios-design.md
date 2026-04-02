# Bluetooth Control Path To iOS Design

Date: 2026-04-02
Status: Design approved, awaiting written spec review

## Scope

This spec defines the Bluetooth control transport for remote control of stock iPhone and iPad devices from Linux and Windows PCs.

The agreed constraints are:

- Target devices are stock iPhone and iPad devices with no jailbreak.
- A one-time manual setup flow on iOS is acceptable.
- The first version must support both iPhone and iPad.
- The control model is hybrid: expose both keyboard and pointer behavior.
- Linux and Windows are both in scope from day one.
- V1 does not promise exact coordinate tap injection.
- The design should leave room for a future coordinate-aware planner above the Bluetooth layer.

## Problem Statement

The system needs a Bluetooth path that lets a Linux or Windows PC act as an input source for iOS. The original idea included screen sharing, coordinate calculation, and action delivery, but this spec is intentionally narrower. It covers only the transport that delivers user or planner intents to iOS over Bluetooth.

On stock iOS, the transport cannot rely on arbitrary low-level input injection. The only viable path is to look like a supported Bluetooth input device and send standard HID-style input that iOS already knows how to accept.

## Goals

- Present the PC to iOS as a Bluetooth input device that stock iOS can pair with.
- Support hybrid control through keyboard and pointer semantics.
- Keep Linux and Windows behavior aligned through a shared action model.
- Fail early and clearly when the host Bluetooth stack or adapter cannot support the required role.
- Preserve a clean interface for future screen-analysis or coordinate-aware planning layers.

## Non-Goals

- Exact tap injection at an arbitrary screen coordinate.
- Jailbreak-only behavior or private iOS APIs.
- Screen capture, screen sharing, or visual target detection.
- App-specific automation guarantees.
- A hardware bridge or external Bluetooth coprocessor in V1.

## User And System Assumptions

- The user can pair the iPhone or iPad with the PC manually.
- The user can enable iOS settings such as AssistiveTouch and keyboard accessibility settings when needed.
- The host PC has a Bluetooth adapter available, but the adapter may not support the required peripheral capabilities.
- Future layers may infer a target position on the iOS screen, but this Bluetooth subsystem only accepts abstract actions and turns them into HID reports.

## Product Definition

The product is a host-native BLE HID peripheral stack that runs directly on Linux and Windows. The PC exposes itself to the iPhone or iPad as a standard Bluetooth input device with two HID roles:

- Keyboard
- Pointer or mouse

The product does not attempt to deliver raw screen-coordinate taps. Instead, it translates higher-level control intents into keyboard and pointer reports that stock iOS interprets using its existing accessibility and external input features.

This makes V1 a HID-semantic control transport, not a pixel-precise automation system.

## Architecture

The system has four runtime layers:

1. Action Planner Interface
2. HID Report Engine
3. BLE Peripheral Adapter
4. Session State

### 1. Action Planner Interface

This is the contract that future operators or planners call into. It accepts abstract intents rather than raw Bluetooth packets. Example intents:

- `PointerMove(dx, dy)`
- `PointerClick(button)`
- `PointerScroll(dx, dy)`
- `KeyPress(code, modifiers)`
- `TextEntry(text)`

This boundary is important because future coordinate-aware logic may exist above this layer without changing the Bluetooth transport. A planner may decide to move toward a target and click, but the Bluetooth layer only guarantees delivery of HID reports.

### 2. HID Report Engine

This layer converts abstract intents into HID reports. It is shared across Linux and Windows.

Responsibilities:

- Report construction for keyboard and pointer roles
- Modifier handling
- Key repeat policy
- Pointer delta clamping
- Scroll behavior
- Text-to-keystroke translation
- Timing and pacing rules

### 3. BLE Peripheral Adapter

This layer owns OS-specific Bluetooth behavior. It is the only major platform-specific part of the design.

Responsibilities:

- Advertising
- Pairing and bonding
- Reconnect behavior
- Service exposure
- Report transport
- Capability probing
- OS-specific error normalization

The exact profile and service implementation details are a backend spike item. The design requirement is functional: iOS must perceive the host as a standard external input device that supports the agreed keyboard and pointer semantics.

### 4. Session State

This layer tracks:

- Paired devices
- Active connection state
- Selected control mode
- Host capability flags
- Device-specific quirks
- Last known successful reconnect metadata

## Platform Components

### Linux Backend

Linux uses BlueZ as the system Bluetooth stack. The Linux backend is responsible for LE advertising, pairing and bonding, service exposure, and report delivery using the selected peripheral implementation path.

This area is explicitly high-risk. BlueZ documents general GATT server and peripheral APIs, but does not provide a simple turnkey path for making an application behave as a polished HOGP device. The Linux backend therefore requires:

- Capability probing at startup
- A dedicated engineering spike before broad implementation claims
- Normalized errors when the selected adapter or stack path cannot support the design

The design assumption is not that Linux support is easy. The design assumption is that Linux is a first-class target with a risky but bounded backend investigation.

### Windows Backend

Windows uses native Bluetooth APIs to determine whether the local adapter supports BLE peripheral role before the product tries to advertise or expose services.

This is also a high-risk path, but the risk is better documented. Peripheral role support is adapter-dependent, so unsupported adapters are part of the expected product matrix, not an edge case.

The Windows backend therefore must:

- Probe capability before starting advertising
- Expose a clear unsupported-adapter state
- Normalize pairing, advertising, reconnect, and transport errors into the shared model

### Shared Input Abstraction

The shared action and HID layers are platform-neutral. This keeps system behavior consistent and limits platform divergence to the radio backend.

That split is the core architectural choice:

- Shared behavior model
- Two OS-specific BLE backends

Not:

- Two separate applications with duplicated input behavior

## Control Modes

The system supports three logical modes:

- Pointer-first
- Keyboard-first
- Hybrid

Hybrid is the default product stance for V1. The host can emit both report types and let the operator or future planner choose whichever input form works for the current iOS screen.

This is necessary because stock iOS behavior varies by context:

- Some screens are easier to navigate with pointer semantics.
- Some screens respond better to keyboard focus movement and activation.
- Some apps may support one mode much better than the other.

## Setup Assistant

Because manual setup is allowed, the product must include a guided setup checklist instead of assuming the user knows which iOS accessibility features to enable.

V1 setup guidance should cover:

- Pairing the iPhone or iPad with the PC
- Pointer setup where AssistiveTouch or related pointer settings are required
- Keyboard accessibility setup such as Full Keyboard Access where needed
- Reconnect expectations for later sessions

## Data Flow

### Startup

1. The app probes host Bluetooth capabilities.
2. If the host cannot support the required peripheral path, the app enters `Unavailable` and reports the reason.
3. If the host is capable, the backend initializes the BLE stack and moves to `ReadyToAdvertise`.

### Pairing

1. The host starts advertising as a composite HID-style input device.
2. The user pairs the iPhone or iPad through iOS Bluetooth settings.
3. On successful bonding, the host stores device metadata and reconnect information.

### Control

Runtime action flow:

`operator or future planner -> abstract action -> HID report engine -> OS BLE backend -> iOS interprets as keyboard or pointer input`

Important boundary:

- The screen-sharing, screen-analysis, or vision system is outside this spec.
- This Bluetooth subsystem exposes an intent-based contract.
- The subsystem does not guarantee a click on exact pixel `(x, y)`.

### Reconnect

After bonding, later sessions should attempt reconnect without full re-pairing. The host uses stored bond metadata and previous session details to restore the connection when the device is available.

## State Model

The Bluetooth subsystem exposes these normalized states:

- `Unavailable`
- `ReadyToAdvertise`
- `Advertising`
- `BondedIdle`
- `Connected`
- `Error`

Definitions:

- `Unavailable`: the host cannot support the required Bluetooth path.
- `ReadyToAdvertise`: adapter and stack are initialized and can begin advertising.
- `Advertising`: waiting for a pair or reconnect.
- `BondedIdle`: previously paired but not currently connected.
- `Connected`: ready to send reports.
- `Error`: a recoverable or actionable failure has occurred.

## Error Handling

Failure classes for V1:

- `HostUnsupported`
- `SetupIncomplete`
- `SessionFailure`
- `BehavioralMismatch`

### HostUnsupported

Examples:

- Windows adapter lacks LE peripheral role support
- Linux stack path cannot expose the required peripheral behavior
- Bluetooth is disabled or unavailable

Response:

- Fail before pairing
- Present exact host capability reason
- Avoid entering a half-configured session state

### SetupIncomplete

Examples:

- Device is paired but expected pointer behavior is unavailable
- Keyboard navigation does not work because required iOS settings were not enabled

Response:

- Surface setup guidance tied to the missing behavior
- Distinguish transport health from effective control

### SessionFailure

Examples:

- Advertising start fails
- Pairing fails
- Bonding fails
- Reconnect fails
- Connection drops during use

Response:

- Return normalized error codes
- Keep enough session logs to reproduce failures
- Allow clean retry paths

### BehavioralMismatch

Examples:

- HID reports are valid but the active iOS screen does not respond as expected
- The current app only partially supports pointer or keyboard control

Response:

- Treat transport success and UI-control success as separate signals
- Log emitted actions and reports for debugging
- Allow mode switching between pointer-first, keyboard-first, and hybrid behavior

## Logging And Diagnostics

The system should log:

- Capability probe results
- Advertising lifecycle transitions
- Pairing and bonding outcomes
- Reconnect attempts
- Abstract actions emitted by upper layers
- HID reports emitted by the shared engine
- Normalized errors and disconnect reasons

This is required because later planner layers will need to debug the gap between "action requested" and "observable iOS response."

## Testing Strategy

### Unit Tests

Cover the platform-neutral action and report logic:

- Action-to-HID translation
- Key rollover behavior
- Modifier timing
- Text entry mapping
- Pointer delta clamping
- Scroll encoding

### Backend Contract Tests

Each OS backend must satisfy the same abstract contract:

- Initialize
- Probe capability
- Advertise
- Pair or bond
- Connect
- Send report
- Disconnect
- Reconnect

Tests verify that backend-specific failures are normalized into the shared error model.

### Manual Device Matrix

Manual testing is required on:

- iPhone
- iPad
- Linux host
- Windows host

Core scenarios:

- First-time pairing
- Reconnect after bonding
- Keyboard-only navigation
- Pointer-only control
- Hybrid mode switching
- Recovery after disconnect

### Acceptance Criteria

V1 is successful when:

- The host can clearly report whether it supports the required Bluetooth path.
- A supported host can pair with a stock iPhone or iPad.
- The bonded device can reconnect in later sessions without full re-pairing.
- The system can deliver keyboard and pointer reports reliably enough for manual control flows that iOS already supports.
- The system surfaces when control failure is due to platform behavior rather than transport failure.

## Risks

- Windows support depends on the local Bluetooth adapter and may vary across hardware.
- Linux support depends on a viable peripheral implementation path on top of BlueZ and should not be treated as solved until backend spikes confirm it.
- iOS behavior is context-sensitive, so a correct HID report does not imply uniform app behavior.
- Hybrid mode improves coverage but increases behavioral test surface.

## Future Compatibility

This design intentionally leaves room for a future coordinate-aware planner above the Bluetooth layer.

Future planners may:

- Analyze a shared screen
- Infer target regions
- Decide whether pointer movement or keyboard navigation is more appropriate
- Emit higher-level actions into the existing transport interface

They still cannot assume exact coordinate injection on stock iOS through this Bluetooth path. Any future coordinate-aware mode remains limited by HID semantics unless Apple-supported mechanisms change.

## References

- Apple Support: Use a pointer device with iPhone, iPad, or iPod touch
  https://support.apple.com/en-us/111775
- Apple User Guide: Full Keyboard Access on iPhone
  https://support.apple.com/en-gw/guide/iphone/ipha4375873f/ios
- Microsoft Learn: Bluetooth developer FAQ
  https://learn.microsoft.com/en-us/windows/uwp/devices-sensors/bluetooth-dev-faq
- Microsoft Learn: GATT server
  https://learn.microsoft.com/en-us/windows/uwp/devices-sensors/gatt-server
- BlueZ documentation: GATT API
  https://bluez.readthedocs.io/en/latest/gatt-api/
- BlueZ documentation: Supported features
  https://bluez.readthedocs.io/en/latest/supported-features/
