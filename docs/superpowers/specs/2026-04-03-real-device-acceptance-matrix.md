# Real-Device Acceptance Matrix

This matrix reflects current reality on this branch. The only verified path today is the local mock plugin-backed flow used by the orchestrator E2E test in the README.

## Acceptance Matrix

| Flow | Capture Path | Control Path | Pairing | Live Preview | Live Control | Recovery | Status |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Local mock flow | Mock capture window | Mock BLE control | N/A | Verified | Not yet verified | Not yet verified | Verified |
| Linux multi-device | Window helper | BLE HID | Pending manual validation | Pending manual validation | Pending manual validation | Pending manual validation | Pending |
| Linux fallback | Window helper | Window input bridge | N/A | Pending manual validation | Pending manual validation | Pending manual validation | Pending |
| Windows multi-device | Window helper | BLE HID | Pending manual validation | Pending manual validation | Pending manual validation | Pending manual validation | Pending |
| Windows fallback | Window helper | Window input bridge | N/A | Pending manual validation | Pending manual validation | Pending manual validation | Pending |

## Operator Validation Checklist

Use this checklist when validating a new real-device flow. Each item should be recorded as Verified or Not yet verified in the matrix above.

1. Host OS and adapters: confirm host OS, Bluetooth adapter, and capture backend availability (window or direct receiver).
2. iOS setup: ensure the device is unlocked, on the same network or Bluetooth session as required, and any permissions dialogs are acknowledged.
3. Pairing: verify BLE pairing or trust prompt completion (if applicable).
4. Live preview: confirm a stable stream of frames with correct orientation and reasonable latency.
5. Live control: confirm keyboard entry and pointer/tap actions are reflected on device.
6. Recovery: verify stop/start session recovery and reconnection behavior after a simulated disconnect.

## Not Yet Verified / Not Yet Supported

These combinations are tracked separately because they are not currently verified on this branch:

- Linux + real iPhone/iPad + window capture
- Linux + real iPhone/iPad + direct receiver
- Windows + real iPhone/iPad + window capture
- Windows + real iPhone/iPad + direct receiver
- Linux + BLE HID control
- Windows + BLE HID control
- Linux + mirrored-window fallback control
- Windows + mirrored-window fallback control

## Planned / Expected

- Full real-device end-to-end flow through the desktop host UI
- Automated reconnection of BLE HID session after host sleep
- Automated capture fallback from window capture to direct receiver
