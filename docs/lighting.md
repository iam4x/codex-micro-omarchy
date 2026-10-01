# Lighting protocol

Every profile has lighting enabled. The editor saves colors and effects immediately.
Brightness and speed preview locally during dragging and save on release, so the
service sends only the final slider value to the device. Both sliders snap to
10% steps, including 0% and 100%. The service checks for
configuration updates every 50 milliseconds.
Omitted `lighting` tables load the default settings. Configuration validation rejects
colors above `0xFFFFFF` and brightness or speed above 100 before replacing the
saved file. The six-element `agents` array fixes the number of individual LEDs.

```toml
[profiles.lighting.keys]
color = 16744192 # #FF8000
brightness = 40
speed = 40
effect = "solid"

[profiles.lighting.ambient]
color = 65535 # #00FFFF
brightness = 50
speed = 40
effect = "snake"
```

The six Agent entries use `[[profiles.lighting.agents]]`. Omit the array to
use six default lights. Defaults are solid `#81A1C1`, 40% brightness and 40%
speed. Effect names in TOML are `off`, `solid`, `snake`, `rainbow`, `breath`,
`gradient`, and `shallow_breath`. The editor hides Snake and Gradient for individual
keys because those effects require a zone with multiple LEDs.

The service owns the HID connection and writes both commands in its device
loop. The config watcher never writes to HID. A new connection starts with
unknown layer state; lighting waits for a successful `device.status` response.
The service sends lighting only on layer index 1. It polls status every three
seconds, so a physical layer change can take that long to be observed.

The service sends `v.oai.rgbcfg` with `keys` and `ambient`, then
`v.oai.thstatus` with all six Agent entries. Both commands use unique request
IDs, separate from the status request ID. Brightness and speed convert from
integer percentages to normalized numbers. The numeric effect codes are 0
through 6 in the order listed above. Color is a packed `0xRRGGBB` integer.
Zone entries set `m` to 0; Agent entries set `sk` and `sa` to 0 so Agent effects
do not replace the other zones.

The service resends after a saved setting changes, after returning to the
Codex layer, and after reconnecting. Unchanged settings send no new frames.
Firmware errors appear in the editor. Missing lighting acknowledgements cause
a reconnect after ten seconds. Acknowledgement proves that firmware accepted
the request; checking the physical LEDs is still needed to confirm the visual
appearance of each effect on a particular firmware version.

Protocol provenance:

- [Work Louder Codex Micro setup](https://worklouder.cc/openai-micro-setup)
  describes six RGB Agent Keys and underglow connection indications.
- [Original Linux protocol research](https://github.com/boopdotpng/work-louder-oai/blob/main/docs/PROTOCOL.md#lighting)
  documents 24-bit colors, the two grouped zones, six individual Agent LEDs,
  and effect codes. It reports physical verification of solid, snake,
  breathing, and gentle breathing on firmware v0.4.1.

Run the model, framing, persistence, and synchronization checks with:

```sh
cargo test --locked
```

Build and verify the real native editor with isolated temporary profiles:

```sh
cargo build --locked
CODEX_MICRO_BINARY="$PWD/target/debug/codex-micro" \
  cargo run --locked --example verify-native -- --lighting
```

This check verifies immediate persistence for all eight targets, restarts the
editor, checks profile isolation and key selection, and preserves binding edits.
It captures Solid, Off, and Breathing to check which controls appear. Its screenshot is `artifacts/native-lighting.png`. It does not
modify the user's saved profiles.
