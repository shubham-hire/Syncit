# Multi-Device Audio Sharing App: Technology, Challenges, Solutions and Feasibility

Prepared for Shubham, October 2026. This is a brainstorming and feasibility document for a fun student project motivated by a genuine sharing problem, not a current business plan. The technical details come from general engineering knowledge plus a small amount of web research on Snapcast, Opus and WebRTC. Nothing here has been tested on real devices, so treat every number as a ballpark and verify it with a prototype.

## 1. Project Summary

**The idea:** a non-Apple application that lets several people hear the same audio on their own devices at once, using Bluetooth headphones, Bluetooth speakers, USB audio devices or wired headphones, with the smallest practical difference in timing between them.

**Two product shapes were explored:**

- **Local routing:** one computer sends audio to several outputs connected to that same computer.
- **Wi-Fi host/listener mode:** any supported non-Apple device can host or listen. The host starts a hotspot, other people join it, and each listener device plays the audio through its own output, including Bluetooth headphones.

The second shape is the one this document focuses on, because it avoids the biggest limit of the first, which is that one device's Bluetooth radio can only serve a few headphones. Apple devices are out of scope for the initial project.

## 2. Summary Table

| Area                             | Conclusion                                                                                                                                                                 |
| -------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Overall feasibility              | Feasible for a supported non-Apple host/listener system, music-first. Platform support must be added incrementally; "any device, any content, zero setup" is not realistic |
| Biggest technical risk           | Unknown and variable Bluetooth latency on each listener's headphones                                                                                                       |
| Biggest environmental risk       | Hotspots that block device-to-device traffic (client isolation)                                                                                                            |
| Biggest platform risk            | Background audio on phones, especially iPhone                                                                                                                              |
| Best MVP                         | Windows host + Android listeners first, with the protocol designed for either role; music only, manual delay slider                                                        |
| Best transport idea              | Opus-encoded audio over the local Wi-Fi network with clock-synchronized playback                                                                                           |
| Best existing tool to learn from | Snapcast (open source, synchronized multi-room audio)                                                                                                                      |
| Main project risk                | Platform differences, hotspot behavior and Bluetooth variance make the experience difficult to reproduce                                                                   |

## 3. How It Works (Simple Version)

```
HOST (laptop)                    LISTENER'S PHONE                 HEADPHONES
plays YouTube/Spotify   -Wi-Fi->  app receives the audio   -BT->   plays the sound
+ our app captures it   hotspot  and plays it through the
                                 phone's normal output
```

1. The host turns on a hotspot and runs the app, which captures system audio. The host may be Windows, Android or another supported non-Apple platform, although Windows is the first target.
2. Listeners join the hotspot and open the app on their phones.
3. Each listener pairs their headphones to their own phone, as they always do. The app is not involved in that step.
4. The host sends timestamped audio over Wi-Fi. Each phone plays it on schedule through its normal output, and its own Bluetooth carries it to the headphones.

**An important correction to the first mental model:** the host does not control the listeners' Bluetooth devices. Bluetooth headphones cannot join a hotspot, because they have no Wi-Fi radio. Each listener's phone is the bridge between Wi-Fi and Bluetooth.

## 4. Technology Overview

### 4.1 Audio capture on the host

- **Windows:** WASAPI loopback capture records whatever the system is playing, and does not need a custom driver. A virtual audio device (needs a signed driver) is only required if you want full control, for example to delay the host's own speakers.
- **Apple platforms:** out of scope for this project. They would require separate capture and background-playback investigation.
- **Android as host:** restricted. Playback capture needs user consent, and apps can opt out of being captured. DRM-protected content is blocked.

### 4.2 Compression: Opus

Opus is a codec designed for real-time audio such as calls, live streaming and broadcasts, and it adapts to available bandwidth. It is mandatory in WebRTC and is the standard choice for low-latency streaming. It also has built-in packet loss concealment, which hides small gaps when packets are lost.

### 4.3 Transport options

| Option                       | Strengths                                        | Weaknesses                                                          |
| ---------------------------- | ------------------------------------------------ | ------------------------------------------------------------------- |
| UDP unicast to each listener | Simple, low latency, predictable                 | Bandwidth grows with listener count; you build reliability yourself |
| WebSocket / TCP              | Easy to implement, works in browsers             | Retransmission can add delay spikes                                 |
| WebRTC                       | Browser listeners with no install, Opus built in | More complex; clock drift handling must be added on top             |
| Multicast                    | Efficient for many listeners                     | Often unreliable on Wi-Fi, so avoid it at first                     |

One research project reported roughly 40 ms audio latency between two MacBooks on a local network using WebRTC. That is a best case from an academic setting. A demo tool I found also states that it does not implement clock drift compensation, so synchronized playback across devices is work you would add.

### 4.4 Synchronization

Snapcast is an open-source client-server multi-room player in which all clients are time-synchronized with the server to play synced audio. It has clients for several platforms, including Android, and Snap.Net for Windows. Its server reads audio from a pipe, so a capture step would be added in front of it. It is the best reference design and a good way to prototype synchronization before writing your own.

A newer protocol called Sendspin appears to offer NTP-style clock sync, jitter buffering and local-network discovery, based on one small Windows client I found. I could not judge its maturity, so treat it as something to investigate.

### 4.5 Listener side

- **Native Android app (recommended):** foreground service for background playback, low-latency audio output (AAudio or Oboe), and control over buffering.
- **Browser page (for demos):** no install, joined by QR code, using the Web Audio API. Sync precision is lower, and background tabs get throttled, especially on iPhone.

### 4.6 Suggested architecture

- **Core engine (C++ or Rust):** ring buffers, per-device delay lines, adaptive resampler, drift controller, mixer. No OS dependency.
- **Platform adapters:** WASAPI on Windows and Oboe/AAudio on Android first; adapters for additional non-Apple platforms can follow.
- **UI layer:** separate from the engine (for example Tauri, Qt or Flutter), reading a per-platform capability matrix so the app shows what each OS allows instead of promising parity.
- **Network layer:** discovery, session tokens, Opus transport, clock sync.

### 4.7 Suggested stack for a first prototype

| Job                           | Technology                                      |
| ----------------------------- | ----------------------------------------------- |
| Capture audio on the host     | WASAPI loopback (Windows)                       |
| Compress                      | Opus                                            |
| Send to listeners             | UDP or WebSocket over the hotspot               |
| Clock sync and timed playback | Snapcast-style approach, or reuse Snapcast      |
| Listener side                 | Android app, or a browser page for a quick test |

The first prototype should implement Windows hosting and Android listening, but the wire protocol and session model should not assume that the host is always Windows. Android hosting and Windows listening can follow once the core synchronization behavior is validated.

## 5. Challenges and Solutions

### 5.1 Summary table

| #  | Challenge                                    | Severity   | Solution                                     | Fully solvable?                                                                        |
| -- | -------------------------------------------- | ---------- | -------------------------------------------- | -------------------------------------------------------------------------------------- |
| 1  | Bluetooth delay on each listener's phone     | High       | Calibration plus manual offset slider        | Partly                                                                                 |
| 2  | Clock sync between host and phones           | High       | NTP-style sync, timestamped playback         | Yes                                                                                    |
| 3  | Wi-Fi jitter and packet loss                 | Medium     | Jitter buffer, Opus loss concealment         | Mostly                                                                                 |
| 4  | Hotspot blocks device-to-device traffic      | High       | Test early, offer fallbacks                  | Depends on hardware                                                                    |
| 5  | Host cannot capture some audio               | Medium     | Loopback capture, accept DRM limits          | Partly                                                                                 |
| 6  | Phones pause audio in the background         | High       | Native app, foreground service               | Mostly solvable on tested Android devices; other platforms require separate validation |
| 7  | Video and audio out of sync                  | High       | Music-first, delay-the-video helper          | Partly                                                                                 |
| 8  | Clock drift over time                        | Medium     | Continuous correction by resampling          | Yes                                                                                    |
| 9  | Joining is too much effort                   | Medium     | QR code, web fallback                        | Mostly                                                                                 |
| 10 | Strangers joining, security                  | Low-Medium | Token in the QR code, host approval          | Yes                                                                                    |
| 11 | Android phone makers killing background apps | Medium     | Foreground service, battery-setting guidance | Mostly                                                                                 |
| 12 | Wi-Fi and Bluetooth sharing the 2.4 GHz band | Medium     | Use 5 GHz for the hotspot                    | Mostly                                                                                 |

### 5.2 Bluetooth delay on each listener's phone

**Problem:** every Bluetooth headphone adds an unknown delay, typically in the range of roughly 100-250 ms, and it differs per device, codec and firmware. The standard Bluetooth audio profile has no reliable way to report the actual figure.

**Solution:**

1. Start with a manual slider on each listener's phone ("nudge earlier or later").
2. Add a calibration beep. The listener hears it from a reference source and from their headphones, and taps when they line up.
3. Later, try automatic calibration with the phone's microphone. It is less reliable, because the microphone often cannot hear the headphones.
4. Save the offset per headphone, so it is a one-time setup.

**Limit:** you cannot get tighter than the delay varies on its own.

### 5.3 Clock sync between host and phones

**Problem:** each device's clock is slightly different, so "play at this time" means different moments on different devices.

**Solution:** each client exchanges several timestamped ping messages with the host and estimates the clock offset, in the same way NTP does. Keep the estimate from the exchange with the lowest round-trip time, since it is the most accurate, and repeat regularly. On a local network this is usually accurate to a few milliseconds, which is well below the Bluetooth uncertainty. Bluetooth, not the clock, is the bottleneck.

### 5.4 Wi-Fi jitter and packet loss

**Problem:** packets arrive at uneven times, or not at all, causing crackling.

**Solution:**

1. A jitter buffer on each phone, so packets are held briefly and played evenly.
2. Opus packet loss concealment, so a lost packet becomes a small glitch, not a dropout.
3. An adaptive buffer size that follows network quality.
4. Optional duplicate or error-correction data for weak connections.

**Trade-off:** you pay in latency for stability. For music, a buffer of roughly 100-300 ms is acceptable. For video, it is not.

### 5.5 Hotspot blocks device-to-device traffic

**Problem:** some hotspots enable client isolation, so phones on the same network cannot reach the host. Some phones also limit how many devices can connect.

**Solution:** test this first. If isolation is on:

- Use a different device as the hotspot (a Windows laptop's Mobile Hotspot often behaves differently from a phone's).
- Use a normal router or shared Wi-Fi.
- As a last resort, add a relay mode through a server, which adds internet dependence and latency.

You cannot fix isolation from inside the app, so this needs documentation and fallbacks.

### 5.6 Host cannot capture some audio

**Problem:** capturing system audio is easiest on Windows and restricted on Android. DRM-protected content may be blocked. Apple platforms are out of scope.

**Solution:** start with Windows loopback capture and add Android hosting later. Tell users plainly which content may not work.

### 5.7 Phones pause audio in the background

**Problem:** when the screen locks, browsers and apps get throttled. This is the biggest practical risk for a web-only version.

**Solution:** use a native app, run playback as an Android foreground service with a media notification, and keep the web version for quick demos with a "keep the screen on" warning. Apple devices are out of scope; each future non-Apple platform still requires its own background-playback validation.

### 5.8 Video and audio out of sync

**Problem:** the buffering that makes streaming reliable means the audio arrives late compared with the picture.

**Solution:** position the product as music-first. For video, add a helper that delays the video by the same amount, which works only where the host controls the player. Games stay out of scope.

### 5.9 Clock drift over time

**Problem:** even synced devices slowly diverge, by tens of parts per million, which becomes audible after some minutes.

**Solution:** each phone measures how far its playback is from the schedule and compensates by resampling very slightly. Listeners cannot hear this. Multi-room systems such as Snapcast do this routinely.

### 5.10 Joining is too much effort

**Problem:** "install an app, join Wi-Fi, enter an address" loses people quickly.

**Solution:** the host shows a QR code that joins the hotspot and opens the stream. A browser page serves first-time guests with no install, and the native app is the upgrade for better playback.

### 5.11 Security

**Problem:** anyone on the network could listen or disrupt the stream.

**Solution:** put a short token or password in the QR code, let the host approve or remove listeners, and encrypt the stream if it carries more than casual music.

### 5.12 Android phone makers and background apps

**Problem:** some brands aggressively kill background apps to save battery, even with a foreground service.

**Solution:** use a foreground service, detect and guide users through battery-optimization settings, and test on the phones your audience actually uses.

### 5.13 Wi-Fi and Bluetooth interference

**Problem:** Wi-Fi and Bluetooth share the 2.4 GHz band, and a crowded band makes both flaky.

**Solution:** prefer a 5 GHz hotspot where the host hardware supports it, and keep listeners reasonably close to the host.

## 6. Feasibility Assessment

### 6.1 By confidence level

**Solved by known engineering (low risk):** clock sync, drift correction, jitter buffering, QR-code joining, basic security. Snapcast-style systems already do most of these.

**Reduced but never fully removed (medium risk):**

- Bluetooth delay. You can calibrate it, but you cannot eliminate it or read it automatically.
- Video sync. Works only where you control the player.
- System audio capture. Easy on Windows, restricted elsewhere.

**Depends on things outside your control (real risks):**

- Hotspot client isolation.
- iPhone background playback.
- Bluetooth variance across headphones.

### 6.2 By platform

| Platform                  | As host                                                          | As listener                                     |
| ------------------------- | ---------------------------------------------------------------- | ----------------------------------------------- |
| Windows                   | Strong: loopback capture works without a driver                  | Possible, lower priority                        |
| Other non-Apple platforms | Requires platform-specific investigation                         | Possible where background playback is supported |
| Android                   | Possible but restricted: playback-capture consent and DRM limits | Strong: foreground service, low-latency audio   |
| Apple devices             | Out of scope for the initial project                             | Out of scope for the initial project            |

### 6.3 Overall verdict

The project is feasible as **a non-Apple host/listener system for music**, with a Windows-host/Android-listener prototype as the first validation path. The long-term goal is for any supported non-Apple device to host or listen, but platform support must be added incrementally. It is not feasible, today, as "any host, any listener, any content, zero setup". Feasible does not mean proven: a prototype is needed to confirm hotspot connectivity, background playback and Bluetooth assumptions.

## 7. Assumptions Worth Challenging

1. **"Minimal delay" conflicts with video and games.** To match a 200 ms Bluetooth device, everything else must be delayed, and video does not wait.
2. **Identical behavior on every platform is unrealistic.** Non-Apple operating systems still impose different limits on capture, networking and background playback.
3. **"Minimal configuration" conflicts with Bluetooth unpredictability.** Calibration is the real user-experience challenge.
4. **Hardware is probably premature.** Validate with software first.
5. **Good-enough solutions already exist for the largest audience.** Samsung Dual Audio, Apple Share Audio and Auracast on newer devices cover many two-headphone cases, so the opportunity lies in the gaps outside those ecosystems.

## 8. Recommended MVP

**Scope:** a non-Apple host/listener product, initially implemented as a Windows host app plus Android listener app, for music and general audio only.

**Features:** capture system audio, encode with Opus, send to listeners, timestamped synchronized playback, manual per-listener delay slider, per-listener volume and mute, QR code joining.

**Deliberately left out:** video sync, games, Apple devices, automatic calibration, equalizer matching.

**Why this scope:** it targets the strongest first validation path while keeping the protocol role-neutral. It tests the three assumptions that can break the idea: hotspot connectivity, background playback and Bluetooth variance. Android hosting and additional non-Apple platforms are follow-on work, not prerequisites for proving the core concept.

## 9. Prototype Plan and Success Criteria

### Step 1: Hotspot test (about one hour)

Create a hotspot, join with two phones and check whether the phones can reach the laptop and each other. If not, test another hotspot source before going further.

### Step 2: Snapcast test (about one day)

Install Snapcast on the laptop and two phones and play music. Listen for echo between devices, and check what happens when the screen locks.

### Step 3: Bluetooth variance test

Connect different headphones to the phones and measure the offset needed for them to line up. Record how much it varies.

### Step 4: Decision

| Result                                                    | Meaning                                          |
| --------------------------------------------------------- | ------------------------------------------------ |
| Hotspot works, sync is acceptable for music with a slider | Continue to a custom MVP                         |
| Hotspot blocks traffic on common phones                   | Redesign around routers or relay, or stop        |
| Background audio keeps dying                              | Prioritize a native Android app, or limit scope  |
| Bluetooth variance is too large to tune by hand           | Invest in calibration, or reconsider the product |

## 10. Users, Use Cases and Project Motivation

**Potential users:** friends or couples watching or listening together with separate headphones, silent-disco style events, gyms and cafes, travelers in hostels or on trains, classrooms, and small venues.

This is currently a fun student engineering project rather than a business proposal. Its value is in solving a genuine everyday problem while exploring real-time audio capture, Opus encoding, local networking, clock synchronization, jitter buffering, Bluetooth delay compensation, background playback and cross-platform design. Monetization, market sizing and commercial positioning are intentionally out of scope until the technical prototype demonstrates that the experience is reliable and easy to use.

## 11. Project Risks and Future Product Risks

- **Native alternatives:** Auracast and phone-maker features are making some versions of the problem standard on newer devices.
- **Setup friction:** requiring an app install and a hotspot on every device limits casual adoption.
- **Support burden:** users will blame the app for Bluetooth physics and hotspot quirks.
- **Platform changes:** capture permissions and background rules can tighten with OS updates.
- **Unproven product direction:** if this later becomes a product, a large audience would not guarantee willingness to pay.

## 12. Open Questions

1. Is the first use case a few of your own devices, or other people joining with their own phones?
2. Which non-Apple platforms should be supported after the first Windows/Android prototype?
3. Which Android phones will you test with? Some brands kill background apps more aggressively than others.
4. Which devices do you own (laptop model, headphones, speakers)? That decides what the first experiment can show.

## 13. Recommended Next Steps

1. Run the hotspot test with your own phones.
2. Run the Snapcast test and write down what you observe.
3. Measure Bluetooth variance with the headphones you have.
4. Decide, using the table in section 9, whether to build the custom MVP.
5. Keep the network protocol host-neutral so a later Android host or other non-Apple host can use the same session model.
