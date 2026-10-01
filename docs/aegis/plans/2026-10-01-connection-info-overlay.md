# Connection Information Overlay Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use aegis:subagent-driven-development (recommended) or aegis:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a persisted lower-left, translucent black diagnostics HUD for an active mirror session.

**Architecture:** Rust appends packet size to the existing IPC event. A pure TypeScript helper calculates one-second samples; `useConnection` owns the counters, `App` owns persistence, and `MirrorScreen` renders the read-only HUD.

**Tech Stack:** Rust, Tauri IPC, React, TypeScript, WebCodecs, Tailwind, Node test runner.

**Baseline / Authority Refs:** `docs/aegis/specs/2026-10-01-connection-info-overlay-design.md`; `src-tauri/src/video.rs`; `src/hooks/useConnection.ts`; `src/components/MirrorScreen.tsx`.

**Compatibility Boundary:** `Packet.size` is additive; old stored settings receive `show_connection_info: false`; TCP packet loss is not displayed.

**Verification:** `node --test tests/*.test.mjs`; `npm run lint`; `npm run build`; manual connect/toggle/reconnect journey.

---

### Task 1: Metric contract and calculation

**Files:** Create `src/lib/connectionMetrics.ts`, `tests/connection-metrics.test.mjs`; modify `src/types.ts`.

**Why this task exists:** Gives bitrate, displayed FPS, decode-drop percentage, and serial/IP fallback a deterministic single owner.

**Impact / Compatibility:** Only additive exports and a new off-by-default Settings property.

- [ ] **Step 1: Write the failing test**

```js
assert.match(source, /export function formatConnectionHost/);
assert.match(source, /export function calculateSample/);
assert.match(source, /bytes \* 8/);
```

- [ ] **Step 2: Run test to verify it fails**

Run `node --test tests/connection-metrics.test.mjs`; expect failure because the helper does not exist.

- [ ] **Step 3: Write minimal implementation**

```ts
export function formatConnectionHost(serial: string) {
  return serial.match(/^\[?([^\]]+)\]?:\d+$/)?.[1] ?? serial;
}
export function calculateSample(bytes: number, frames: number, dropped: number, targetFps: number) {
  return { bitrateMbps: bytes * 8 / 1_000_000, fps: frames, dropRate: frames + dropped ? dropped / (frames + dropped) * 100 : null, targetFps };
}
```

Add `show_connection_info`, additive packet `size`, and overlay model types in `src/types.ts`.

- [ ] **Step 4: Run test to verify it passes**

Run `node --test tests/connection-metrics.test.mjs`; expect PASS.

- [ ] **Step 5: Commit**

Run `git add src/lib/connectionMetrics.ts src/types.ts tests/connection-metrics.test.mjs` then `git commit -m "feat(metrics): 定义连接诊断采样"`.

### Task 2: IPC packet size and connection sampling

**Files:** Modify `src-tauri/src/video.rs`, `src/hooks/useConnection.ts`, `src/types.ts`; test `tests/connection-metrics.test.mjs`.

**Why this task exists:** Supplies actual throughput and decoder/render pressure instead of configured values.

**Impact / Compatibility:** Base64 payload and prior frame events remain unchanged; size is additive.

- [ ] **Step 1: Write the failing test**

```js
assert.match(rust, /Packet \{ key: bool, data: String, timestamp: u64, size: usize \}/);
assert.match(hook, /msg\.data\.size/);
assert.match(hook, /droppedFrames/);
```

- [ ] **Step 2: Run test to verify it fails**

Run `node --test tests/connection-metrics.test.mjs`; expect contract assertions to fail.

- [ ] **Step 3: Write minimal implementation**

```rust
Packet { key: is_key, data: STANDARD.encode(&avcc), timestamp: pts, size }
```

In the hook, add packet bytes before decode; count decoder output frames; count replacement of an unpainted pending frame as a decode drop; sample/reset counts every second; expose queue size, dimensions, codec, and session duration; clear on cleanup.

- [ ] **Step 4: Run test to verify it passes**

Run `node --test tests/connection-metrics.test.mjs`; expect PASS.

- [ ] **Step 5: Commit**

Run `git add src-tauri/src/video.rs src/hooks/useConnection.ts src/types.ts tests/connection-metrics.test.mjs` then `git commit -m "feat(stream): 采集实时连接指标"`.

### Task 3: Persist switch and present HUD

**Files:** Modify `src/App.tsx`, `src/components/SettingsDialog.tsx`, `src/components/MirrorScreen.tsx`; test `tests/connection-metrics.test.mjs`.

**Why this task exists:** Delivers the approved user journey: enable in Settings, then see a non-interactive lower-left black translucent card while connected.

**Impact / Compatibility:** HUD is off for current users; existing centered recording and macro controls stay unchanged. Retire the existing adaptive mini-badge and include tier/profile in the HUD footer.

- [ ] **Step 1: Write the failing test**

```js
assert.match(app, /show_connection_info/);
assert.match(settings, /Show connection overlay/);
assert.match(mirror, /pointer-events-none/);
assert.match(mirror, /bg-black\/75/);
```

- [ ] **Step 2: Run test to verify it fails**

Run `node --test tests/connection-metrics.test.mjs`; expect HUD assertions to fail.

- [ ] **Step 3: Write minimal implementation**

```tsx
{showConnectionInfo && connectionInfo && (
  <div className="absolute bottom-2 left-2 pointer-events-none bg-black/75 backdrop-blur-lg rounded-md px-2.5 py-2 text-[10px] text-white font-mono z-5">
    {/* host, five metric rows, codec/profile/duration */}
  </div>
)}
```

Merge `show_connection_info: false` into initial settings; add a `Connection Info` switch using `onUpdateSetting`; pass metrics to `MirrorScreen`; remove the old `adaptiveInfo` display branch.

- [ ] **Step 4: Run tests and quality checks**

Run `node --test tests/*.test.mjs; npm run lint; npm run build`; expect all commands to exit 0. Then manually connect, toggle setting, confirm values update every second and clicks pass through, disable it, reconnect, and verify persistence.

- [ ] **Step 5: Commit**

Run `git add src/App.tsx src/components/SettingsDialog.tsx src/components/MirrorScreen.tsx tests/connection-metrics.test.mjs` then `git commit -m "feat(ui): 添加连接信息悬浮窗"`.

## Risks and rollback

Current tests are source-level Node tests, so the required manual journey is the UI evidence floor. The IPC field adds one integer per packet; if that is profiled as expensive, batch telemetry in a future contract change. Roll back by removing the additive size field and the disabled HUD preference.
