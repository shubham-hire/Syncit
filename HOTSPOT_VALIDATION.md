# Phase 6 hotspot validation runbook

This document is the operational checklist for the first real-network
listener test (Roadmap Milestone 2). Record every run in the results table;
do not advance to multi-listener work until one listener is stable.

## Setup checklist

1. **Enable Windows Mobile Hotspot**: Settings → Network & Internet →
   Mobile hotspot → On. Note the network name/password from the phone.
2. **Find the host's hotspot address** (usually `192.168.137.1`):

   ```powershell
   Get-NetIPAddress -AddressFamily IPv4 |
     Where-Object { $_.InterfaceAlias -like "*Local Area*" -or $_.InterfaceAlias -like "*Hotspot*" } |
     Select-Object InterfaceAlias, IPAddress
   ```

3. **Open the UDP port** (replace `41000` with your `--bind` port; run in an
   elevated PowerShell):

   ```powershell
   New-NetFirewallRule -DisplayName "AudioSync host UDP" -Direction Inbound `
     -Protocol UDP -LocalPort 41000 -Action Allow -Profile Private,Public
   ```

4. **Connect the listener machine/phone** to the hotspot Wi-Fi.

## Run matrix

Perform each step in order; only advance when the previous one passes.

| # | Source | Output | Network | Pass criteria |
|---|--------|--------|---------|---------------|
| 1 | synthetic | null | localhost (same PC) | `rendered` grows at ~50/s, `forger 0` |
| 2 | synthetic | null | hotspot (2 devices) | same, RTT reported < 20 ms typical |
| 3 | synthetic | speakers/wired | hotspot | tone audible and continuous |
| 4 | WASAPI | wired | hotspot | host's system audio plays on listener |
| 5 | WASAPI | Bluetooth | hotspot | same, note added latency/concealment |
| 6 | WASAPI | wired | hotspot, 10-minute `--csv` soak | zero or bounded concealment, no stalls |

Host command (Windows):

```powershell
cargo run -p audiosync-apps --bin audiosync-host --locked -- --bind 0.0.0.0:41000
```

Listener command (second device, replace session/token from the host print):

```bash
cargo run -p audiosync-apps --bin audiosync-listener --locked -- \
  --host 192.168.137.1:41000 --session <hex32> --token <hex32> \
  --volume 0.8 --csv soak.csv
```

Add `--null-output` for steps 1–2 and `--duration-secs 600` for step 6.

## What to record per run

```text
date / time:
host OS + Wi-Fi band:
listener device + OS:
output device (wired/Bluetooth model):
step number:
rendered / concealed frames:
duplicates / late / unauthenticated packets:
RTT (µs) and offset stability:
dropouts noticed by ear:
--buffer-ms used (default 100):
notes:
```

## Interpreting results

- **Concealed frames climbing**: raise `--buffer-ms` (150, 200) until it
  stops; the difference is the network's real jitter budget.
- **`forger` > 0 with no explanation**: another process is emitting on the
  port; investigate before continuing (the auth tag rejected it — that is
  the protocol v2 protection working).
- **Listener never becomes Active**: host firewall or hotspot client
  isolation. Verify with `Test-NetConnection 192.168.137.1 -Port 41000`
  (TCP-only check; UDP has no built-in tool — the step-1→2 jump is the UDP
  probe). If client isolation is confirmed, PRD §12 fallback: use a
  dedicated travel router instead of Mobile Hotspot.
- **RTT spikes but audio continuous**: acceptable; the clock estimator keeps
  the lowest-RTT sample.

## Go/no-go (PRD §12)

Continue to multi-listener work only when: one listener stays stable for the
10-minute soak with ≤1 audible interruption and concealed frames bounded.

## Results

| date | step | devices | result | notes |
|------|------|---------|--------|-------|
| (pending first hardware run) | | | | |
