# Product Requirements Document: Local Audio Sharing

## 1. Document status

- **Status:** Draft for student-project prototype
- **Date:** October 2026
- **Primary objective:** Validate a genuine local audio-sharing problem with a reliable, easy-to-join prototype.
- **Business objective:** None currently. Commercial viability is intentionally out of scope.

## 2. Product vision

Create a non-Apple application that allows one person to host an audio session and other people to listen to the same audio on their own devices and headphones. The host creates the local Wi-Fi network; listeners connect directly to that host and play synchronized audio through their normal device output.

The long-term product goal is role flexibility: any supported non-Apple device should be able to act as either host or listener. The first implementation will deliberately support only the strongest validation path rather than all platforms at once.

## 3. Problem statement

When several people want to hear the same audio, they often need to share headphones, use platform-specific sharing features, or tolerate different audio sources. Existing solutions do not consistently support mixed devices, independent Bluetooth headphones, or a host-controlled local session.

This project investigates whether a simple local network can make shared listening practical without requiring every listener to use the same headphones, phone brand or ecosystem.

## 4. Target users and use cases

### Primary users

- Students and friends sharing music or general audio.
- Small groups using their own phones and headphones.
- Developers learning about real-time audio and networking.

### Initial use cases

1. One person hosts music from a Windows laptop.
2. Two to five people connect Android phones to the host hotspot.
3. Each listener uses their own wired or Bluetooth headphones.
4. The group adjusts individual delay when headphone latency differs.
5. The session continues while listener screens are locked.

### Out of scope for the first prototype

- Apple devices.
- Games and interactive low-latency audio.
- Guaranteed video/audio synchronization.
- DRM circumvention.
- Automatic Bluetooth pairing.
- Internet relay mode.
- Commercial billing or monetization.

## 5. Product principles

- **Host-neutral protocol:** the session model must not permanently assume that Windows is the host.
- **Local-first:** the host hotspot should be sufficient; internet access is not required.
- **Honest capability reporting:** the app must explain platform limitations rather than promise universal support.
- **Measured synchronization:** use recorded timing data instead of subjective claims alone.
- **Simple joining:** a listener should not need to type an IP address.

## 6. Roles and core flow

### Host

The host:

1. Starts a session.
2. Enables or configures a Wi-Fi hotspot.
3. Captures local system audio.
4. Encodes the audio with Opus.
5. Sends timestamped packets to approved listeners.
6. Displays connected listeners and their status.
7. Can remove or mute a listener.

### Listener

The listener:

1. Connects to the host's hotspot.
2. Scans a QR code or enters a short session code.
3. Requests or receives host approval.
4. Synchronizes its clock and playback timeline.
5. Buffers and decodes the stream.
6. Plays through the selected device output.
7. Adjusts local volume and delay.

### Role flexibility

The protocol must support either role conceptually. Initial acceptance does not require every platform to implement both roles. Windows hosting and Android listening are the first target; Android hosting and additional non-Apple platforms are subsequent milestones.

## 7. Functional requirements

### Must have for the validation MVP

- Host can create a local session.
- Host can capture supported system audio.
- Host can advertise the session on its hotspot.
- Listener can join without manually entering an IP address.
- Host can see connected listeners.
- Host can approve or remove listeners.
- Audio is encoded and streamed using Opus.
- Packets contain sequence and timing information.
- Listener has a bounded jitter buffer.
- Listener plays audio in the background on supported Android devices.
- Each listener has an independent delay adjustment.
- Each listener has volume and mute controls.
- Short packet loss does not permanently stop playback.
- Listener can leave and rejoin the session.

### Should have after the first successful prototype

- Android can host a session.
- Windows can act as a listener.
- Automatic local host discovery.
- Reconnection after a short Wi-Fi interruption.
- Persisted delay settings per headphone or output device.
- A basic calibration workflow.
- A documented device compatibility matrix.

### Could have later

- Linux support.
- Browser listener for demonstrations.
- More than five listeners.
- Video delay assistance where the host controls the video player.
- Automatic acoustic calibration.

## 8. Non-functional requirements

- The first prototype must work without internet access after dependencies are installed.
- The listener should not require the screen to remain unlocked during normal Android playback.
- Session audio should not be stored permanently by default.
- Session tokens must prevent unauthorized casual joining.
- The app should provide clear errors for hotspot isolation, unsupported capture, missing permissions and lost connections.
- The protocol should be documented well enough to implement a second host or listener platform later.

## 9. Initial technical direction

| Area | Initial direction |
|---|---|
| User interface | Flutter application shared across Windows, Android and later non-Apple platforms |
| Shared engine | Rust library for protocol, timing, buffering and codec integration |
| Host | Flutter Windows application with a Windows/WASAPI loopback adapter |
| Listener | Flutter Android application with a native foreground playback service |
| Audio codec | Opus, initially 48 kHz with a tested frame size and bitrate |
| Transport | Timestamped local-network packets; start with a controlled UDP prototype or reuse Snapcast for validation |
| Synchronization | NTP-style clock-offset estimation and scheduled playback |
| Drift correction | Small adaptive resampling adjustments |
| Joining | Host hotspot plus QR code/session token |
| Security | Per-session random token, host approval and authenticated session traffic |

Flutter owns screens, controls, QR/session presentation and capability messages.
It must not own the real-time audio pipeline: Android playback must continue in
a foreground service when the Flutter activity is paused, and Windows capture
must use a platform adapter. The Rust engine should be exposed to Flutter
through a narrow generated FFI/API bridge rather than exposing internal packet
buffers directly to Dart.

The transport decision should be confirmed through the validation spike. Do not build a complete custom protocol before confirming that hotspot connectivity, background playback and Bluetooth variation are acceptable.

## 10. Timing model and measurable targets

The system must distinguish between:

1. Host capture-to-network latency.
2. Network and jitter-buffer latency.
3. Phone-output-to-headphone latency.

Bluetooth latency is expected to vary by headphone and codec. The product should synchronize the stream timeline and compensate for known per-listener offsets; it cannot guarantee identical end-to-end delay for every headphone.

### Initial success targets

- Two to five listeners in one session.
- At least 30 minutes of continuous playback.
- Listener-to-listener timing difference of approximately 20–50 ms after manual calibration.
- No more than one noticeable audio interruption during a 30-minute normal-network test.
- Listener can join in under two minutes without entering an IP address.
- Playback survives screen locking on tested Android devices.
- A short network interruption results in recovery within approximately two seconds.

These are prototype targets, not promises to users.

## 11. Validation plan

### Experiment 1: Hotspot connectivity

- Use the intended Windows host and at least two Android phones.
- Verify that each phone can reach the host.
- Check whether client isolation prevents host-to-listener traffic.
- Record hotspot device, Wi-Fi band, phone models and Android versions.

### Experiment 2: Existing synchronized-audio reference

- Test Snapcast or an equivalent reference system.
- Check synchronization, screen-lock behavior and reconnection.
- Use the result to estimate whether the problem is worth implementing independently.

### Experiment 3: Bluetooth variance

- Test several headphone and speaker combinations.
- Measure the offset required to align two listeners.
- Repeat after reconnecting devices and changing codecs where possible.
- Record whether manual calibration remains stable.

### Experiment 4: Prototype load

- Test two, five and then more listeners if the first results are stable.
- Measure host CPU, memory, network bandwidth and dropout rate.
- Identify the practical listener limit for the selected host hardware.

## 12. Acceptance criteria for continuing

Continue to a custom MVP when:

- The host hotspot allows reliable communication.
- At least two listeners can remain connected for 30 minutes.
- Background playback works on the selected Android test devices.
- Manual delay adjustment produces acceptable music synchronization.
- The join flow can be completed without technical instructions.

Pause or redesign when:

- Common host hotspots isolate listeners from the host.
- Screen-locked playback repeatedly stops.
- Bluetooth variance cannot be made acceptable with manual adjustment.
- The setup requires too many manual network steps.
- Host resource use becomes impractical at the intended listener count.

## 13. Risks and mitigations

| Risk | Mitigation |
|---|---|
| Hotspot client isolation | Test early; support normal routers or another hotspot source as documented fallbacks |
| Bluetooth latency variation | Manual delay slider first; calibration later |
| Android background process termination | Foreground service, media notification and battery-setting guidance |
| DRM or protected audio | State capture limitations clearly; do not attempt circumvention |
| Packet loss and Wi-Fi jitter | Jitter buffer, sequence numbers, Opus concealment and bounded recovery |
| Clock drift | Periodic clock estimation and small resampling corrections |
| Unauthorized listeners | Per-session token and host approval |
| Platform fragmentation | Maintain a capability matrix and add platforms incrementally |

## 14. Success definition

The project is successful if it demonstrates that a non-Apple host can create a local session, multiple non-Apple listeners can join through the host hotspot, and the group can hear music with acceptably small timing differences using their own audio devices. A commercial business model is not required for success.

## 15. Open decisions

- Which non-Apple platforms should be implemented after Windows and Android?
- Should the first transport be a small custom UDP protocol or a Snapcast-based experiment?
- What exact synchronization threshold feels acceptable to test participants?
- Which phone, headphone and hotspot combinations define the supported test matrix?
- Should Android hosting be included in the first public demonstration or only after the Windows-host path is stable?
