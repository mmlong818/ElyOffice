import { test } from "node:test";
import assert from "node:assert/strict";
import { BEAT, bar } from "../video/type/draw.js";
import { nameCubes } from "../video/type/scenes.js";
import { moment } from "../video/scenes/timeline.ts";
import { recording, slabs } from "../video/scenes/turn.ts";
import { grown } from "../video/scenes/parts.ts";
import recs from "../video/montage/recs.json" with { type: "json" };

const APPS = ["Doc", "Table", "Show", "PDF", "Tree", "Mark"];

test("each app's name clears before its screen flattens", () => {
  APPS.forEach((name, i) => {
    const b = 9 + 4 * i;
    // The screen flattens on the bar's last beat.
    const flat = bar(b) + 3 * BEAT;
    assert.ok(nameCubes(`Ely${name}`, b, flat - BEAT).some((c) => c.a > 0), `Ely${name} shows first`);
    assert.ok(nameCubes(`Ely${name}`, b, flat).every((c) => c.a === 0), `Ely${name} is gone`);
  });
});

test("leaving screens keep their place and play as they go", () => {
  assert.deepEqual(slabs, recs.apps.map(({ app }) => grown(app)), "the montage's own screens");
  for (let f = 2186; f < 2215; f++) {
    const shown = moment(f / 30).pieces.filter(({ box, land }) => slabs.includes(box) && land > 0);
    assert.ok(shown.every(({ box, land }) => recording(box, land, f)), `frame ${f}`);
    if (f === 2186) assert.equal(shown.length, 6, "all six cross the cut");
  }
});
