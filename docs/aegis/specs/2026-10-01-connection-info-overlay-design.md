# Connection information overlay

## Intent

Add a persisted setting that shows a compact, read-only connection diagnostics HUD while an Android device is mirrored. The HUD helps users identify stream quality problems without opening developer tools or obscuring device input.

## Confirmed design

- Add a `Show connection overlay` switch in a new `Connection Info` section of Settings.
- Persist the switch with the existing `stream_settings` object; it defaults to off for existing users.
- When enabled and connected, render a black, translucent, blurred overlay at the lower-left of the mirror viewport. It must ignore pointer events.
- Present connection address, live video bitrate, displayed/target FPS, displayed resolution, decoder queue depth, decode drop rate, codec, active quality mode, and session duration.
- Refresh live values once per second. Hide the HUD while disconnected and while a stream is being reconfigured.

## Metrics and definitions

| HUD label | Definition | Source |
| --- | --- | --- |
| IP | Network host parsed from a wireless ADB serial when available; otherwise the device serial | Connected device metadata |
| Bitrate | Bytes received in the previous rolling second, converted to Mbps | Video packet payload bytes |
| FPS | Frames emitted by `VideoDecoder` in the preceding second / configured target FPS | Decoder output + settings |
| Resolution | Current decoded frame display width x height | Canvas/video frame dimensions |
| Decoder queue | Current `VideoDecoder.decodeQueueSize` | WebCodecs |
| Dropped frames | Percentage of decoded frames superseded before canvas paint during the sampling interval | Pending-frame rendering path |
| Codec / profile | Active video codec and selected quality mode | Decoder config + stream settings |

TCP guarantees delivery and does not expose packet retransmissions to the app, so the overlay deliberately does not label the dropped-frame metric as network packet loss. This prevents presenting an inaccurate network-loss value.

## Visual contract

```text
┌──────────────────────────────┐
│ ● CONNECTED   192.168.1.24   │
│                              │
│  码率       7.8 Mbps         │
│  帧率       58 / 60 FPS      │
│  分辨率     1080 × 2400      │
│  解码队列   1 frame          │
│  丢帧率     0.3%             │
│                              │
│  H.264 · Balanced · 00:12:48 │
└──────────────────────────────┘
```

The card uses a semi-transparent black surface (`black/75` range), backdrop blur, compact monospaced values, and the existing small overlay typography. It occupies the lower-left of the viewport, replacing the current adaptive-quality indicator so no two diagnostics panels compete for that position. Recording and macro-recording controls remain centered and unobstructed.

## Architecture and data flow

1. The Rust video forwarding layer includes packet byte length with each frontend packet event.
2. `useConnection` aggregates received bytes, decoded frames, paint-superseded frames, current decoded dimensions, connection time, and decoder queue depth in a small metrics hook/state interface.
3. `App` owns the persisted visibility setting and supplies metrics plus configured stream settings to `MirrorScreen`.
4. `MirrorScreen` owns presentation only through a dedicated, non-interactive overlay component.

## Error handling and compatibility

- No IP extraction failure blocks the overlay: it falls back to the serial value.
- Values unavailable before the first decoded frame display as an em dash rather than zero.
- Existing saved settings merge with the new default so old local-storage values remain valid.
- Adaptive mode retains its existing behavior; its tier name is included in the HUD instead of its prior separate badge.

## Testing

- Unit-test metrics aggregation: byte-rate window, FPS sampling, drop-rate calculation, IP fallback, and default-setting compatibility.
- Add a component/integration test for: setting off => no HUD; setting on + connected => HUD renders expected labels and values.
- Run TypeScript build, lint, and relevant tests after implementation.

## Non-goals

- Measuring TCP-level packet loss, retransmissions, RTT, or Wi-Fi signal strength.
- Making the HUD draggable, resizable, or interactive.
- Adding historical graphs or recording telemetry.
