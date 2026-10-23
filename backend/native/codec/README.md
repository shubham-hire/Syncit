# SyncIt Opus codec

This crate owns the first audio-frame contract:

- 48 kHz sample rate
- Mono PCM
- 20 ms frames
- 960 samples per frame
- 96 kbps initial encoder target

`OpusEncoder` accepts exactly one PCM frame and returns one encoded packet.
`OpusDecoder` returns a fixed-size PCM frame and supports
`decode_loss()` for Opus packet-loss concealment after a previously decoded
frame.

The crate uses the Rust `opus` bindings and builds the vendored native Opus
library through CMake. Keep codec state in one worker per stream; the encoder
and decoder are stateful and should not be shared concurrently without an
owner-managed synchronization boundary.
