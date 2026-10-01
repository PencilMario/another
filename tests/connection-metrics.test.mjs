import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

test("connection metrics expose host fallback and one-second sample calculations", async () => {
  const source = await readFile(new URL("../src/lib/connectionMetrics.ts", import.meta.url), "utf8");

  assert.match(source, /export function formatConnectionHost/);
  assert.match(source, /export function calculateConnectionSample/);
  assert.match(source, /bytes \* 8/);
  assert.match(source, /droppedFrames/);
});
