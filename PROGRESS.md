# AudioSync Progress Snapshot

**Recorded:** October 25, 2026, 18:54:45 (+05:30)  
**Repository:** `D:\Audio_sync\Syncit`  
**Branch:** `main`  
**Base revision:** `15668f0` (`Succesfully created a session and joined a listener`)  
**Working tree:** Local uncommitted changes are present; do not discard unrelated edits.

## Goal

Validate a real Windows AudioSync session end to end:

```text
Windows WASAPI loopback host
  -> approved UDP listener
  -> persistent Windows audio output
```

The current acceptance target is a host on one Windows PC and a listener on a
second Windows PC, preferably with the listener using headphones or another
separate output device.

## Current state

### Working

- Flutter Windows host and listener pages load the Rust FFI engine.
- Host approval works, including the pending-listener banner and Approve/Reject
  dialog.
- The host can select either:
  - real `WASAPI loopback` capture, or
  - a generated `synthetic tone` for transport/output diagnostics.
- Release Windows listeners use a persistent CPAL/WASAPI output worker instead
  of a per-pump null sink.
- Listener status exposes packet, jitter, timing, sink, watchdog, and rejoin
  diagnostics.
- The first live packet timestamp is rebased when the first listener becomes
  active. This prevents approval/setup time from making the first packets appear
  late.
- Host runtime errors and retained host events are visible in the host UI.
- Session/token identifiers remain 32-character hexadecimal strings; they are
  not parsed as integers.

### User-verified observations

#### Synthetic mode

The user observed:

```text
Host diagnostics: running true
Source: synthetic tone
Events: HeartbeatReceived and ClockSyncRequestReceived
Listener: Session connected
received increasing
inserted increasing
buffer stable around 7 packets
sink failures 0
```

The audible steady buzz is intentional: the synthetic source is a 220 Hz sine
wave at 48 kHz with amplitude 12,000. A stable buffer and zero sink failures
indicate that the synthetic transport, scheduling, decoding, and output path
are functioning.

#### Same-PC real WASAPI mode

The user heard duplicate audio in non-synthetic mode while host and listener
were using the same PC/default Windows output. This is expected behavior:

1. WASAPI loopback captures the audio already playing through the host output.
2. The listener plays the captured stream through that same output.
3. The original audio and delayed relayed copy are therefore both audible.

This is not evidence of duplicate UDP packets. A clean real-audio test requires
the listener on a second PC or a different output device.

#### Earlier timing symptom and fix

Before timestamp rebasing, the listener showed:

```text
late increasing
buffer 0
inserted increasing slowly
clock offset approximately 34,541 ms
capacity drops 0
```

The large clock offset was plausible because each process has its own monotonic
clock origin. The actual defect was that the host timestamp seed was created at
host startup, potentially long before listener approval. The first live packet
was consequently treated as stale. The timestamp rebase fixed this path; the
synthetic retest then showed a stable buffer around 7 and zero sink failures.

## Important implementation changes

- `backend/native/apps/src/cpal_sink.rs`
  - Persistent CPAL output and Windows sample-format conversion.
- `backend/native/audio/src/lib.rs`
  - `AudioPacketPump::start_at_micros` for first-frame timestamp rebasing and
    regression coverage.
- `backend/native/ffi/src/json.rs`
  - Correct JSON writer finalization without the legacy trailing comma.
- `backend/native/ffi/src/lib.rs`
  - Persistent listener sink wiring, source label, packet/playback metrics, and
    FFI regression checks.
- `backend/native/runtime/src/lib.rs`
  - Rebase the first audio timestamp at the first active send and test it.
- `frontend/flutter-app/lib/engine/engine_controller.dart`
  - Parse host errors/events and detailed listener metrics.
- `frontend/flutter-app/lib/features/host/host_session_page.dart`
  - Synthetic-source switch, host diagnostics, approval UI, and same-PC WASAPI
    caution.
- `frontend/flutter-app/lib/features/listener/listener_join_page.dart`
  - Detailed listener diagnostics and same-PC loopback warning.
- `frontend/flutter-app/test/engine_controller_test.dart`
  - Event, host-status, listener-metric, and source-label parsing coverage.
- `README.md`, `CHANGELOG.md`, and `frontend/flutter-app/lib/engine/README.md`
  - Updated runtime, diagnostics, and same-device testing documentation.

## Checks completed

All commands below were run from `D:\Audio_sync\Syncit` unless noted.

### Rust tests and quality

```text
cargo test --workspace --locked --offline
PASS — all workspace tests passed

cargo test -p audiosync-audio -p audiosync-runtime --locked --offline
PASS — audio and runtime tests passed

cargo clippy --workspace --all-targets --locked --offline -- -D warnings
PASS

cargo fmt --all -- --check
PASS

cargo build --workspace --locked --offline
PASS

cargo build -p audiosync-ffi --release --locked
PASS — target/release/audiosync_ffi.dll rebuilt

cargo run -p audiosync-sim --locked --offline
PASS — AudioSync simulator PASS
```

### Flutter checks

Run from `D:\Audio_sync\Syncit\frontend\flutter-app`:

```text
flutter test --no-pub
PASS — 9 tests passed

flutter analyze --no-pub
PASS — No issues found

dart format --output=none --set-exit-if-changed lib test
PASS
```

### Repository hygiene

```text
git diff --check
PASS
```

No commit or push was performed. The working tree contains the existing local
changes listed by `git status`; preserve them.

## Next manual test

Use the second Windows PC now available.

### Host PC

1. Rebuild/reload the release DLL if the host process has not been restarted:

   ```powershell
   cd D:\Audio_sync\Syncit
   cargo build -p audiosync-ffi --release --locked
   ```

2. Start the Flutter host from `frontend\flutter-app`.
3. Leave **Use synthetic test tone** disabled.
4. Enable the Windows Mobile Hotspot or use a private network shared by both
   PCs.
5. Allow UDP port `41000` if needed:

   ```powershell
   New-NetFirewallRule `
     -DisplayName "AudioSync host UDP" `
     -Direction Inbound `
     -Protocol UDP `
     -LocalPort 41000 `
     -Action Allow `
     -Profile Private,Public
   ```

6. Find the host's hotspot/LAN IPv4 address:

   ```powershell
   Get-NetIPAddress -AddressFamily IPv4 |
     Where-Object {
       $_.InterfaceAlias -like "*Local Area*" -or
       $_.InterfaceAlias -like "*Hotspot*"
     } |
     Select-Object InterfaceAlias, IPAddress
   ```

7. Start the host, then copy its session and token to the second PC.

### Listener PC

1. Connect to the host's hotspot/private network.
2. Start the Flutter Windows listener.
3. Enter `<HOST_IP>:41000`, the session, and the token.
4. Do **not** use `127.0.0.1`; that points to the listener PC itself.
5. Approve the listener from the host PC.
6. Use headphones or another output on the second PC.

Record the listener values after 30–60 seconds:

```text
received
inserted
rendered
concealed
buffered
duplicate
late
unauthenticated
sink failures
RTT / clock offset
```

Expected separate-PC result:

```text
received: increasing
inserted: increasing
rendered: increasing steadily, approximately 50 frames/second
buffer: stable and non-zero, typically several packets
concealed: stable or near 0
late: stable or near 0
duplicate: 0
unauthenticated: 0
sink failures: 0
```

If this passes, continue with wired and Bluetooth listener outputs and then a
10-minute soak test. If duplicate audio is still heard on the second PC, record
the metrics above; that would be a real streaming issue rather than the known
same-PC loopback effect.

## Known gaps

- The Linux sandbox cannot perform Windows WASAPI, CPAL hardware, Flutter
  Windows, hotspot, or multi-device audio validation.
- The Flutter app currently targets Windows and Android; macOS/Apple devices are
  out of scope and are not a supported listener target.
- Android PCM output and foreground playback service are not implemented.
- Native volume and mute controls are still UI-only.
- Host address discovery and QR join payloads are not implemented.
- Stronger control-message authentication and broader multi-listener soak
  testing remain open.
