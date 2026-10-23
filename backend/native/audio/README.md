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
use audiosync_audio::{OpusEncoderPipeline, WasapiCapture};

let source = WasapiCapture::default_loopback()?;
let mut encoder = OpusEncoderPipeline::new(source)?;
let opus_packet = encoder.next_packet()?;
```

## Sources

- `SyntheticPcmSource` is deterministic and is used by the offline simulator's
  regression scenario.
- `WasapiCapture` is available on Windows and captures the default render device
  through shared-mode WASAPI loopback. It requests a 48 kHz stereo float stream,
  downmixes it to mono signed 16-bit PCM, and supplies codec-sized frames.

On non-Windows targets the `WasapiCapture` type remains available for portable
call sites, but construction returns an explicit unsupported-platform error.
The Linux simulator and unit tests never require an audio device.

For cloning the repository on Windows, running the current checks, and tracking
the next live-capture steps, see
[`WINDOWS_WASAPI_TESTING.md`](WINDOWS_WASAPI_TESTING.md).
