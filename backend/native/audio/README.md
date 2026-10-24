# SyncIt audio sources

This crate connects PCM producers to the stateful `OpusEncoder` without making
the codec depend on an operating-system API.

## PCM contract

`PcmSource` supplies one frame at a time using the codec's fixed format:

- 48 kHz
- mono
- signed 16-bit PCM
- 20 ms / 960 samples per frame

`OpusEncoderPipeline<S>` owns a `PcmSource` and an `OpusEncoder`. Calling
`next_packet()` reads one PCM frame and returns one encoded Opus packet.

On Windows, the host-side connection is intentionally small:

```rust,no_run
use audiosync_audio::{AudioPacketPump, WasapiCapture};
use audiosync_core::SessionId;

let source = WasapiCapture::default_loopback()?;
let mut pump = AudioPacketPump::new(source, SessionId::default(), 0)?;
let audio_packet = pump.next_packet()?;
```

`AudioPacketPump` is platform-neutral: it owns the stateful encoder, assigns
contiguous sequence numbers, and timestamps each packet at 20 ms intervals. It
does not open sockets or decide which listeners are active; the transport layer
owns UDP fan-out.

## Sources

- `SyntheticPcmSource` is deterministic and is used by the offline simulator's
  regression scenario.
- `WasapiCapture` is available on Windows and captures the default render
  device for the Multimedia role through shared-mode WASAPI loopback. It
  requests a 48 kHz stereo float stream, downmixes it to mono signed 16-bit
  PCM, and supplies codec-sized frames.

On non-Windows targets the `WasapiCapture` type remains available for portable
call sites, but construction returns an explicit unsupported-platform error.
The Linux simulator and unit tests never require an audio device.

## Windows capture smoke test

The `wasapi_capture_smoke` example exercises the real Windows loopback path in
memory: it opens the default render endpoint, captures a configurable number of
20 ms frames, encodes every frame with the `OpusEncoderPipeline`, decodes every
packet with a matching `OpusDecoder`, and verifies that every decoded frame has
exactly 960 mono PCM samples. It reports capture, packet, packet-size,
non-silence, timeout, and elapsed-time statistics without writing a recording.

Run it from the repository root while ordinary audio is playing through the
default Windows output device:

```powershell
cargo run -p audiosync-audio --example wasapi_capture_smoke --release --locked -- --frames 500
```

The example defaults to 500 frames when `--frames` is omitted. A successful
run exits with code 0 and prints `WASAPI capture smoke test PASS`; WASAPI,
capture, encoding, decoding, and timeout errors exit nonzero with context.

The current implementation phases and their validation boundaries are recorded
in the repository-level [`IMPLEMENTATION_PHASES.md`](../../../IMPLEMENTATION_PHASES.md).

For cloning the repository on Windows, running the current checks, and tracking
the next live-capture steps, see
[`WINDOWS_WASAPI_TESTING.md`](WINDOWS_WASAPI_TESTING.md).
