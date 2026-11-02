import { test } from "node:test";
import assert from "node:assert/strict";
import { order } from "../src/iso.ts";
import { ACCENT, model } from "../src/model.ts";

const all = model.flatMap((p) => p.blocks);
const [desk] = model[0].blocks;

test("the model draws without intersections or cycles", () => {
  assert.equal(order(all).length, all.length);
});

test("every block stands on the desk", () => {
  for (const b of all.slice(1)) {
    assert.ok(b.x >= 0 && b.y >= 0 && b.x + b.w <= desk.w && b.y + b.d <= desk.d, JSON.stringify(b));
    assert.ok(b.z >= desk.h);
  }
});

test("the accent covers at most a tenth of the visible tops", () => {
  const area = (list: typeof all) => list.reduce((s, b) => s + b.w * b.d, 0);
  assert.ok(area(all.filter((b) => b.tone === ACCENT)) / area([desk]) <= 0.1);
});

test("seven apps between the desk and the roads", () => {
  assert.deepEqual(model.slice(1, -1).map((p) => p.name), ["ElyDoc", "ElyTable", "ElyShow", "ElyPDF", "ElyMail", "ElyTree", "ElyMark"]);
});

type Box = (typeof all)[number];
const near = (a: number, aw: number, b: number, bw: number) => a <= b + bw + 1e-9 && b <= a + aw + 1e-9;
const shared = (a: number, aw: number, b: number, bw: number) => Math.min(a + aw, b + bw) - Math.max(a, b) > 1e-9;
/** Two ground blocks meet along an edge or overlap. */
const meet = (a: Box, b: Box) =>
  a.z === b.z && near(a.x, a.w, b.x, b.w) && near(a.y, a.d, b.y, b.d) &&
  (shared(a.x, a.w, b.x, b.w) || shared(a.y, a.d, b.y, b.d));

const part = (name: string) => {
  const found = model.find((p) => p.name === name);
  assert.ok(found, name);
  return found.blocks;
};

/** Whether roads join two apps' ground blocks. */
function linked(from: string, to: string): boolean {
  const roads = part("Roads");
  const ground = (name: string) => part(name).filter((b) => b.z === roads[0].z);
  const start = roads.filter((r) => ground(from).some((b) => meet(r, b)));
  const seen = new Set(start);
  for (const r of seen) for (const n of roads) if (!seen.has(n) && meet(r, n)) seen.add(n);
  return [...seen].some((r) => ground(to).some((b) => meet(r, b)));
}

test("every app has a building", () => {
  for (const p of model.slice(1, -1)) assert.ok(p.blocks.length > 0, p.name);
});

test("roads carry exports to PDF and Markdown to Git", () => {
  for (const app of ["ElyDoc", "ElyTable", "ElyShow", "ElyMark"]) assert.ok(linked(app, "ElyPDF"), app);
  assert.ok(linked("ElyMark", "ElyTree"));
});
