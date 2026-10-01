import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

test("toolbar pinned actions are restored instead of cleared during app startup", async () => {
  const appSource = await readFile(new URL("../src/App.tsx", import.meta.url), "utf8");

  assert.match(appSource, /localStorage\.getItem\("pinned_actions"\)/);
  assert.doesNotMatch(appSource, /localStorage\.removeItem\("pinned_actions"\)/);
});
