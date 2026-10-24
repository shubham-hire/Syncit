# Windows WASAPI testing guide

## Short answer

Yes. You can clone this repository on a Windows machine and run the Rust
workspace checks there.

The current checkout supports two different levels of validation:

1. **Build and regression validation:** available now. The workspace compiles
   the Windows WASAPI adapter, and the simulator exercises the synthetic
   PCM-to-Opus path without requiring an audio device.
2. **Live system-audio validation:** available through the checked-in
   `wasapi_capture_smoke` example. It constructs `WasapiCapture`, captures
   real default-render loopback frames, round-trips them through Opus, and
   reports the result without writing audio to disk.

`audiosync-sim` remains deliberately synthetic and does not access the Windows
audio device. The smoke example is the separate hardware validation path.

## Windows prerequisites

Install the following on the Windows host:

- Git.
- Rustup and the Rust MSVC toolchain.
- CMake, because the Opus dependency builds its native library locally.
- Visual Studio Build Tools with:
  - **Desktop development with C++**.
  - A Windows 10 or Windows 11 SDK.
  - The C++ CMake tools included in the workload, or a separate CMake
    installation on `PATH`.

Use the MSVC target for this project. A normal PowerShell session works when
the Visual Studio tools are on `PATH`; the **Developer PowerShell for Visual
Studio** is the safest option when diagnosing compiler or linker errors.

Verify the tools before cloning or building:

```powershell
git --version
rustup --version
cmake --version
```

The repository pins Rust in `rust-toolchain.toml`. The first Cargo command may
ask Rustup to download Rust `1.99.1` and the matching components.

## Clone and select the pinned toolchain

Replace `<repository-url>` with the repository's Git URL:

```powershell
git clone <repository-url> AudioSync
Set-Location AudioSync

rustup show active-toolchain
```

The active toolchain should contain `1.99.1` and an MSVC target such as
`x86_64-pc-windows-msvc`. If Rustup has not installed the pinned toolchain yet,
run:

```powershell
rustup toolchain install 1.99.1
```

Do not delete or replace `Cargo.lock`. All commands below use `--locked` so a
clean clone cannot silently resolve a different dependency graph.

## Checks that work immediately

Run these commands from the repository root:

```powershell
cargo test --locked
cargo run -p audiosync-sim --locked
cargo build --workspace --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
```

The simulator should finish with output containing:

```text
SyncIt simulator PASS
```

These checks cover:

- The fixed 48 kHz mono Opus encoder and decoder.
- The `PcmSource` and `OpusEncoderPipeline` contracts.
- The deterministic synthetic source.
- Audio packet serialization, jitter buffering, packet loss concealment, and
  session behavior.
- Compilation of the workspace on Windows.

The `cargo test` run does not need a microphone, speakers, or a running audio
application. The synthetic simulator also does not use WASAPI.

## Run the real WASAPI capture smoke test

While ordinary audio is playing through the default Windows output device, run
this exact command from the repository root:

```powershell
cargo run -p audiosync-audio --example wasapi_capture_smoke --release --locked -- --frames 500
```

The example captures 500 20 ms frames by default, encodes every captured frame,
decodes every generated packet using the pipeline's codec configuration, and
verifies exactly 960 mono PCM samples per decoded frame. It keeps all samples
and packets in memory. A successful run prints, at minimum:

```text
WASAPI capture smoke test PASS
frames captured: 500
packets encoded: 500
packets decoded: 500
maximum Opus packet size: ... bytes
non-silent frames: ...
WASAPI timeouts: 0
elapsed time: ... seconds
```

Initialization, capture, encoding, decoding, and timeout errors are reported on
stderr and return a non-zero process exit code.

## Compile the Windows adapter explicitly

The Windows source is conditionally compiled, so an explicit target check is a
useful way to confirm that the real adapter—not the non-Windows placeholder—is
being compiled:

```powershell
cargo check -p audiosync-audio --target x86_64-pc-windows-msvc --locked
```

This validates the WASAPI bindings, COM setup, loopback stream initialization,
polling-based capture, float-stereo-to-mono conversion, and cleanup code. It
does not open the default render endpoint or prove that live audio is arriving.

## What is not wired yet

The smoke example validates the local capture and codec path only. It does not
run a long-lived host runtime, join or approve listeners, or send packets over
a real hotspot. The platform-neutral `AudioPacketPump`, local UDP
`AudioPacketFanout`, and session-aware `audiosync-runtime` now exist and are
covered by local UDP tests, but they are not yet connected to a Windows hotspot
or an Android playback service.

## Next implementation plan

### Step 1: Windows capture smoke example — complete

The Windows-aware example is now checked in at:

```text
backend/native/audio/examples/wasapi_capture_smoke.rs
```

The example:

1. Construct `WasapiCapture::default_loopback()`.
2. Construct `OpusEncoderPipeline<WasapiCapture>`.
3. Capture a configurable number of frames, defaulting to 500 frames / 10
   seconds.
4. Decode each generated packet with `OpusDecoder` to verify the encoded data.
5. Report frame count, packet count, maximum packet size, elapsed time, and
   whether any captured frame was non-silent.
6. Keep samples in memory only; do not write an audio recording by default.
7. Exit nonzero with a clear error if the endpoint cannot be opened or a frame
   times out.

The command is:

```powershell
cargo run -p audiosync-audio --example wasapi_capture_smoke --release --locked -- --frames 500
```

Smoke-test acceptance criteria:

- The default render endpoint opens successfully.
- 500 capture frames are read without a timeout or device error.
- Every frame produces a non-empty Opus packet.
- Every packet decodes to 960 mono PCM samples.
- Playing a known tone or ordinary system audio produces non-silent frames.
- Running with no active playback still produces valid frames or a documented
  silent-stream result.

### Step 2: Add the host audio packet pump — complete at library level

`AudioPacketPump<S>` now owns one `OpusEncoderPipeline<S>` and produces
`AudioPacket` values.

The pump should:

- Assign one monotonically increasing sequence number per 20 ms frame.
- Use a sample-based timestamp timeline:
  `capture_start_micros + frame_index * 20_000`.
- Preserve one encoder instance for the lifetime of the stream.
- Return source and codec errors to the host runtime instead of panicking.

The host runtime, rather than this synchronous pump, is responsible for adding
the bounded scheduling/queue boundary needed to keep a slow network from
blocking a live capture worker.

The pump is in `audiosync-audio`, depends only on the platform-neutral codec and
core contracts, and does not put Windows APIs into `audiosync-session` or
`audiosync-core`.

### Step 3: UDP fan-out and simulator reuse — complete at local-test level

The simulator now uses the same packet-pump path:

```text
SyntheticPcmSource
  → OpusEncoderPipeline
  → host audio packet pump
  → AudioPacket
  → AudioPacketFanout / simulated loss and reordering
  → JitterBuffer
  → OpusDecoder
```

`AudioPacketFanout` sends one encoded packet to every supplied endpoint and
records individual destination failures. The simulator also sends three
synthetic packets to two local UDP listeners and decodes both copies.

### Step 4: Session-aware host runtime — complete at local-runtime level

`audiosync-runtime` now connects the control socket, `HostSession`,
`AudioPacketPump`, and `AudioPacketFanout` in a scheduler-driven host runtime.
Its local test joins and approves a listener, answers clock sync, and sends
synthetic audio only after the listener is active.

The runtime does not create a thread or sleep; the eventual Windows host calls
its control and audio methods from its own scheduler. Hotspot validation and UI
approval wiring remain outstanding.

### Step 5: Reference listener pipeline — complete at local-runtime level

`ListenerAudioPipeline` now validates UDP audio packets, tracks session and
packet metrics, reorders packets through the bounded jitter buffer, decodes
ready frames, and produces explicit Opus concealment frames when the caller
declares a packet late. `ListenerRuntime` owns the UDP receive socket and
returns decoded PCM frames to its caller. A local 500-packet host-to-listener
test passes through UDP and Opus decoding with zero concealment frames.

It does not yet provide an Android foreground service or platform audio sink.

### Step 6: Validate one real hotspot listener — next

Run synthetic audio over the Windows hotspot first, then replace the source with
WASAPI capture. Test a wired output and a Bluetooth output separately.

### Step 7: Add multiple listeners and timing calibration

Measure listener-to-listener skew, Bluetooth output delay, packet loss and
recovery before expanding to the two-to-five-listener target.

### Step 8: Device and stream lifecycle hardening

After the receiver path works, handle the operational cases:

- Default output device unavailable at startup.
- Output device removal or replacement while capturing.
- WASAPI event timeout.
- Stream discontinuity.
- Host session stop and clean encoder shutdown.
- Clear user-facing errors for unsupported or unavailable capture.

### Step 9: Measure before starting Android playback work

Record at least:

- Capture-to-encoded-packet latency.
- CPU use during continuous capture.
- Packet size distribution.
- Behavior during silence and normal playback.
- Recovery after changing the Windows output device.

Only after the host can produce a stable real Opus packet stream and one
receiver can play it should the Android playback bridge and Flutter FFI be
connected. See `IMPLEMENTATION_PHASES.md` for the complete phase record.

## Suggested Windows test record

For each hardware run, record:

```text
Windows version:
Rust toolchain:
Audio output device:
Device format shown by Windows:
Audio application used for the test:
Frames requested / captured:
Non-silent frames:
Maximum Opus packet bytes:
Timeouts or discontinuities:
Elapsed time:
```

Avoid committing recordings, device identifiers, or private audio to the
repository.

### Current checkout test record

The live WASAPI smoke test was run three times from native Windows PowerShell
using the same command, with a different default Windows output category for
each run. All three runs satisfied the 500-frame acceptance criteria.

Test command:

```powershell
cargo run -p audiosync-audio --example wasapi_capture_smoke --release --locked -- --frames 500
```

| Run | Default output used | Frames captured | Packets encoded / decoded | Non-silent frames | Maximum Opus packet | WASAPI timeouts | Elapsed time |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | Laptop speakers | 500 | 500 / 500 | 500 | 409 bytes | 0 | 10.033 s |
| 2 | Wired headphones | 500 | 500 / 500 | 500 | 391 bytes | 0 | 10.027 s |
| 3 | Bluetooth headset | 500 | 500 / 500 | 500 | 396 bytes | 0 | 10.056 s |

The three runs all printed `WASAPI capture smoke test PASS`. No capture,
encoding, decoding, timeout, or device errors were observed during these
runs. The first run compiled the example in 19.80 seconds; the second and
third runs reused the completed build and started in 0.13 and 0.19 seconds,
respectively.

Windows version and exact hardware make/model were not recorded. The tested
output categories were laptop speakers, wired headphones, and a Bluetooth
headset as labeled above.

The earlier event-driven implementation had failed at frame 1 with
`Handle wait timed out`. The current polling-based implementation using the
Multimedia render role completed all three native Windows runs without that
failure.

After the runtime changes, a smoke-test recheck was also attempted in the
current execution environment. It stopped at frame 1 with:

```text
WASAPI loopback produced no capture packet within 1000 ms
```

That recheck did not have a confirmed active playback/device context and is not
counted as a replacement for the three successful hardware records above. Run
the command again from native Windows PowerShell while ordinary audio is
playing before treating a new result as a hardware regression.

## Troubleshooting

### `link.exe` or MSVC compiler not found

Open a Visual Studio Developer PowerShell, or install the Desktop development
with C++ workload and Windows SDK. Confirm that `cargo` is using an MSVC target:

```powershell
rustc -vV
rustup target list --installed
```

### CMake is not found

Install CMake and reopen PowerShell so the updated `PATH` is visible:

```powershell
cmake --version
```

### The simulator passes but no live audio is heard

The simulator is synthetic and does not access the Windows audio device. Use
the live WASAPI smoke command above, confirm that Windows has an active default
render endpoint, and make sure a normal audio application is playing through it.

### Capture is silent

Check the selected Windows default output device and test with ordinary system
audio. Some protected or restricted content may not be available through
loopback capture; the project does not attempt to bypass those restrictions.

### `WASAPI loopback produced no capture packet within ... ms`

The adapter uses shared-mode polling rather than relying on a loopback event
callback, because some Windows endpoint/driver combinations do not signal the
event handle even while the render endpoint is active. Confirm that ordinary
audio is playing through the selected default output device and rerun the smoke
command. The timeout remains intentional so an unavailable or inactive endpoint
returns a clear non-zero error instead of waiting forever.
