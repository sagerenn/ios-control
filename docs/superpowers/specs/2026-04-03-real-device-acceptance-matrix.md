# Real-Device Acceptance Matrix

This matrix reflects current reality on this branch. Rows marked "Not yet verified" are not validated end-to-end yet. The only verified path today is the local mock plugin-backed flow used by the orchestrator E2E test in the README.

## Acceptance Matrix

| Flow | Capture Path | Control Path | Pairing | Live Preview | Live Control | Recovery | Status |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Local mock flow | Mock capture window | Mock BLE control | N/A | Verified | Verified | Verified | Verified |
| Linux + real iPhone/iPad + window capture | Window capture plugin | BLE HID control | Not yet verified | Not yet verified | Not yet verified | Not yet verified | Not yet verified |
| Linux + real iPhone/iPad + direct receiver | Direct receiver plugin | BLE HID control | Not yet verified | Not yet verified | Not yet verified | Not yet verified | Not yet verified |
| Windows + real iPhone/iPad + window capture | Window capture plugin | BLE HID control | Not yet verified | Not yet verified | Not yet verified | Not yet verified | Not yet verified |
| Windows + real iPhone/iPad + direct receiver | Direct receiver plugin | BLE HID control | Not yet verified | Not yet verified | Not yet verified | Not yet verified | Not yet verified |
| Linux + BLE HID control | N/A | BLE HID control | Not yet verified | N/A | Not yet verified | Not yet verified | Not yet verified |
| Windows + BLE HID control | N/A | BLE HID control | Not yet verified | N/A | Not yet verified | Not yet verified | Not yet verified |

## Operator Validation Checklist

Use this checklist when validating a new real-device flow. Each item should be recorded as Verified or Not yet verified in the matrix above.

1. Host OS and adapters: confirm host OS, Bluetooth adapter, and capture backend availability (window or direct receiver).
2. iOS setup: ensure the device is unlocked, on the same network or Bluetooth session as required, and any permissions dialogs are acknowledged.
3. Pairing: verify BLE pairing or trust prompt completion (if applicable).
4. Live preview: confirm a stable stream of frames with correct orientation and reasonable latency.
5. Live control: confirm keyboard entry and pointer/tap actions are reflected on device.
6. Recovery: verify stop/start session recovery and reconnection behavior after a simulated disconnect.

## Not Yet Supported (Planned / Expected)

These are intentionally tracked separately from the acceptance matrix and are not verified on this branch.

- Full real-device end-to-end flow through the desktop host UI
- Automated reconnection of BLE HID session after host sleep
- Automated capture fallback from window capture to direct receiver
