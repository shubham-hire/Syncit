# Windows WASAPI testing guide

## Short answer

Yes. You can clone this repository on a Windows machine and run the Rust
workspace checks there.

The current checkout supports two different levels of validation:

1. **Build and regression validation:** available now. The workspace compiles
   the Windows WASAPI adapter, and the simulator exercises the synthetic
   PCM-to-Opus path without requiring an audio device.
2. **Live system-audio validation:** the adapter code is present, but the
   repository does not yet contain a command-line capture smoke test that
   constructs `WasapiCapture`. The next implementation step is to add that
   small executable before integrating capture with network packet sending.

In other words, cloning on Windows is useful now, but `audiosync-sim` is still
deliberately synthetic and does not access the Windows audio device.

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

## Compile the Windows adapter explicitly

The Windows source is conditionally compiled, so an explicit target check is a
useful way to confirm that the real adapter—not the non-Windows placeholder—is
being compiled:

```powershell
cargo check -p audiosync-audio --target x86_64-pc-windows-msvc --locked
```

This validates the WASAPI bindings, COM setup, loopback stream initialization,
event handling, float-stereo-to-mono conversion, and cleanup code. It does not
open the default render endpoint or prove that live audio is arriving.

## What is not wired yet

There is currently no checked-in command equivalent to:

```text
open default render endpoint
→ capture real PCM
→ encode N Opus frames
→ decode them for validation
→ print capture statistics
```

`WasapiCapture` is a library type, and the simulator intentionally uses
`SyntheticPcmSource`. Do not interpret a successful simulator run as proof that
Windows loopback capture has been tested.

## Next implementation plan

### Step 1: Add a Windows capture smoke example

Add a Windows-aware example at:

```text
backend/native/audio/examples/wasapi_capture_smoke.rs
```

The example should:

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

The planned command is:

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

### Step 2: Add the host audio packet pump

Create a platform-neutral pump that owns one
`OpusEncoderPipeline<S: PcmSource>` and produces `AudioPacket` values.

The pump should:

- Assign one monotonically increasing sequence number per 20 ms frame.
- Use a sample-based timestamp timeline:
  `capture_start_micros + frame_index * 20_000`.
- Preserve one encoder instance for the lifetime of the stream.
- Use a bounded queue so a slow network does not block the capture worker.
- Return source and codec errors to the host runtime instead of panicking.

The first implementation can use the existing `UdpTransport` and send packets
to one approved listener. It should not put Windows APIs into
`audiosync-session` or `audiosync-core`.

### Step 3: Reuse the same pump in the simulator

Replace the simulator's direct pipeline setup with the same packet-pump path:

```text
SyntheticPcmSource
  → OpusEncoderPipeline
  → host audio packet pump
  → AudioPacket
  → simulated loss/reordering
  → JitterBuffer
  → OpusDecoder
```

This keeps the synthetic simulator as a regression test for the exact packet
path used by Windows capture.

### Step 4: Add device and stream lifecycle handling

After the smoke test works, handle the operational cases:

- Default output device unavailable at startup.
- Output device removal or replacement while capturing.
- WASAPI event timeout.
- Stream discontinuity.
- Host session stop and clean encoder shutdown.
- Clear user-facing errors for unsupported or unavailable capture.

### Step 5: Measure before starting Android playback work

Record at least:

- Capture-to-encoded-packet latency.
- CPU use during continuous capture.
- Packet size distribution.
- Behavior during silence and normal playback.
- Recovery after changing the Windows output device.

Only after the host can produce a stable real Opus packet stream should the
Android playback bridge and Flutter FFI be connected.

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

That is expected with the current repository: the simulator is synthetic, and
the live WASAPI smoke command is the next planned change. Once the smoke command
exists, confirm that Windows has an active default render endpoint and that a
normal audio application is playing through it.

### Capture is silent

Check the selected Windows default output device and test with ordinary system
audio. Some protected or restricted content may not be available through
loopback capture; the project does not attempt to bypass those restrictions.
