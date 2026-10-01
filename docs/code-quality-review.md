# Code quality review

Reviewed on 2026-10-01 using the thermo-nuclear-code-quality-review skill.
This records the review before the Rust-only cleanup. The Python prototype,
its packaging and tests, and the icon generator have since been removed.
Native UI verification now runs with `cargo run --locked --example verify-native`.

Scope: all Rust modules, the Python prototype, tests, verification and install
scripts, assets, configuration examples, and service and udev definitions.

## Findings and fixes

| Finding | Fix | Evidence |
| --- | --- | --- |
| The 878-line UI module mixed window rendering, editing, service connections, and test automation. | Split editing, rendering, and event handling into focused modules. The UI core is now 240 lines. Modules import their own dependencies explicitly. | Rust compilation, Clippy, and native UI verification. |
| A mutable dirty flag contradicted the saved form. Reloading saved text marked it as edited. | Remove the flag and compare the current draft with the selected saved binding. Phase changes keep the form intact. | Reproduced against the installed baseline; the native check now asserts saved and reloaded forms are clean. |
| Profile writes skipped validation and shared one temporary filename. Invalid data could replace valid bindings; concurrent writes could collide. | Validate before writing; use a unique temporary file, sync it, atomically persist it, and sync the containing directory. Reject unknown profile fields and invalid profiles at editor startup. | Invalid saves preserve the previous file. Eight concurrent writers produce valid complete profiles and leave no temporary files. |
| Configuration reloads depended on a connected device and modification timestamps. | Watch file contents independently of HID connections. Keep the last valid profile after malformed edits and derive mapped control counts from that profile. | Tests cover initial loading, unchanged timestamps, deletion, malformed edits, recovery, and status without hardware. |
| Service requests and responses used ad hoc JSON shapes. The UI automation endpoint acknowledged invalid operations, and Test could acknowledge a failed process spawn. | Typed requests, replies, and snapshots; shared bounded newline framing; explicit errors for invalid requests and failed launches. | Real Unix socket tests cover malformed, incomplete, and invalid requests. Framing tests preserve multiline Unicode above the old 64 KiB limit. |
| Full subscriber queues silently discarded status updates. Closed editors left idle service connections waiting for hardware events. | Disconnect overflowing subscribers so they reconnect with current status. Add idle status checks and scoped subscription cleanup. | Queue pressure and live subscription tests, including idle peer shutdown and registration cleanup. |
| Shortcut capture removed an arbitrary held key when a release did not match. Shift changes could also turn repeats into false multiple-key errors. | Match known shifted key pairs; ignore unrelated releases; preserve the recorded chord until its own keys and modifiers are released. | The unrelated-release regression failed before the fix. Tests now cover modifier ordering, shifted names, repeats, cancellation, and multiple keys. |
| Every launched action allocated a waiting thread, including long-running applications. | Reap action processes through the existing smol executor and report start and exit events. Prefer the installed systemd service when starting the daemon. | A real process test checks start and nonzero exit reporting; native verification checks service execution. |
| Theme parsing accepted invalid RGB lengths, and adding icons required maintaining a second manual registry. | Require six hexadecimal RGB digits. Generate the embedded icon list from the asset directory at build time. | RGB validation tests, unchanged regenerated SVG bytes, and native rendering. |
| The Python prototype installed the same command as the Rust application, and socket setup failures leaked its signal handler. | Rename it to `work-louder-prototype` and place worker startup and cleanup inside the socket lifecycle. Clearly label its separate configuration schema. | Five Python tests pass, including startup failure and SIGTERM cleanup. The installed prototype validates its example file. |
| CLI arguments after the first were ignored; installation reused a fixed staging filename and started the service twice. | Reject excess arguments, use a unique binary staging file, and enable then restart the service once. Native verification positions its window using monitor geometry. | CLI checks, shell syntax checking, installation, and native verification. |
| The Python socket test opened real HID hardware and could compete with the running native service. | Mock only hardware discovery in the subprocess while keeping its real socket server, worker lifecycle, and signal handling. | Socket subscription and SIGTERM cleanup still pass without opening the Micro. |

No source file exceeds 1,000 lines. Existing volume/media commands, device
layout, multiline text, shortcut recording, and phase-switching behavior are
preserved.

## Verification

- Rust: 27 unit tests; formatting and Clippy with warnings denied.
- Python: 5 tests; Ruff linting and formatting; locked dependency checks.
- Build: locked release build and byte-identical regenerated SVG assets.
- Native app: full smoke check passes against both the release build and the
  installed application with the restarted native service. It covers input
  dimensions, search, multiline text, saved-form state, shortcut releases,
  phase switching, action execution, reset/undo, removal, and media assignments.
- CLI: excess arguments and invalid saved profiles are rejected without
  changing the invalid file.
- Installation: installed binary matches the release build; the user service
  is active; the Micro is connected over Bluetooth; saved bindings validate.

Native automation exercises GPUI callbacks and keyboard events using a
temporary profile. HID framing and microphone coalescing are covered by unit
tests. Keyboard capture was checked against this machine's English (US)
layout. Joystick direction mapping remains outside the implemented feature
set, as documented in the README.
