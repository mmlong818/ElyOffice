// The film's voxel titles, timed on sixteenths.
import { cells } from "./glyphs.js";
import { BAR, BEAT, SIX, bar, bounds, clamp, cubes, settle, text, view } from "./draw.js";

const DARK_FIELD = "#252525";
const LIGHT = "#e9e9e6";
const INK = "#4d4d51";
const ACCENT = "#3d7fe5";
const SOFT = "#6e6e73";
const BODY = "#48484a";
const YAW = (12 * Math.PI) / 180;
const GAP = 0.06;
const LAND = 0.2;

/** Cubes of `s`; `tone` picks a tone per glyph. */
function word(s, tone) {
  return cells(s).cells.map((c) => ({ ...c, tone: tone(c.glyph) }));
}

/** A cell's box, raised `dz`, height scaled `h`. */
function box(c, dz = 0, a = 1, h = 1) {
  const s = 1 - 2 * GAP;
  return { x: c.col + GAP, y: GAP, z: 6 - c.row + GAP + dz, w: s, d: s, h: s * h, tone: c.tone, a };
}

/** Each cell's slot: a rising sweep, `n` sixteenths wide. */
function slots(list, n, lean = 0.5) {
  const key = (c) => c.col + lean * (6 - c.row);
  const keys = list.map(key);
  const [lo, hi] = [Math.min(...keys), Math.max(...keys)];
  return list.map((c) => Math.min(n - 1, Math.floor(((key(c) - lo) / (hi - lo + 1e-6)) * n)));
}

/** Drop one cell, landing at `land`; null before. */
function arrive(t, land) {
  const p = (t - (land - LAND)) / LAND;
  if (p <= 0) return null;
  const e = settle(clamp(p));
  return { dz: 1 - e, a: e };
}

/** Lift one cell and fade, ending at `end`. */
function lift(t, end, span) {
  const e = settle(clamp((t - (end - span)) / span));
  return { dz: e, a: 1 - e };
}

/** View placing `list` with its bounds at an anchor. */
function place(list, cell, { cx, left, cy }) {
  const b = bounds(view(YAW, cell), list.map((c) => box(c)));
  const ox = left !== undefined ? left - b.x0 : cx - (b.x0 + b.x1) / 2;
  return view(YAW, cell, ox, cy - (b.y0 + b.y1) / 2);
}

/** Scales view `v` by `s` about screen point (px, py). */
function zoom(v, s, px, py) {
  return view(YAW, v.cell * s, px + (v.ox - px) * s, py + (v.oy - py) * s);
}

/** Screen y just below the word, left of screen `x`. */
function under(v, x) {
  let c = 0;
  while (v.p(c, 1, -2)[0] < x) c++;
  return v.p(c, 1, -2)[1];
}

/** Small stable noise per cell. */
const hash = (c, k) => {
  const x = Math.sin(c.col * 127.1 + c.row * 311.7 + k * 74.7) * 43758.5453;
  return x - Math.floor(x);
};

/** Bars 1–2: ELY builds, then breaks into falling tiles. */
function ely(ctx, t) {
  ctx.fillStyle = DARK_FIELD;
  ctx.fillRect(0, 0, 1920, 1080);
  const list = word("ELY", () => LIGHT);
  const last = list.find((c) => c.col === 14 && c.row === 6);
  last.tone = ACCENT;
  const rest = list.filter((c) => c !== last);
  const slot = new Map(rest.map((c, i) => [c, slots(rest, 20)[i]]));
  slot.set(last, 23);
  const end = 2 * BAR;
  const u = clamp(t / end);
  const v = zoom(place(list, 46, { cx: 960, cy: 520 }), 1.42 - 0.42 * (1 - (1 - u) ** 1.6), 960, 540);
  const boxes = list.flatMap((c) => {
    const at = arrive(t, (slot.get(c) + 1) * SIX);
    if (!at) return [];
    const start = (28 + Math.floor((c.col / 17) * 4)) * SIX;
    const τ = t - start;
    if (τ <= 0) return [box(c, at.dz, at.a)];
    const flat = settle(clamp(τ / 0.1));
    const b = box(c, -48 * τ * τ, 1, 1 - 0.78 * flat);
    b.x += (hash(c, 1) - 0.5) * 5 * τ;
    b.y += (0.4 + hash(c, 2)) * 9 * τ * τ;
    return [b];
  });
  cubes(ctx, v, boxes);
}

/** Bars 7–8: "Ely Office" stands in the lower third. */
function title(ctx, t) {
  const t0 = bar(7);
  const end = bar(9);
  const list = word("Ely Office", () => INK);
  const slot = slots(list, 6);
  const v = place(list, 22, { cx: 960, cy: 780 });
  const left = bounds(v, list.map((c) => box(c))).x0;
  const gone = lift(t, end, SIX);
  const boxes = list.flatMap((c, i) => {
    const at = arrive(t, t0 + (slot[i] + 1) * SIX);
    return at ? [box(c, at.dz + gone.dz, at.a * gone.a)] : [];
  });
  cubes(ctx, v, boxes);
  const a = settle(clamp((t - (bar(8) - LAND)) / LAND)) * gone.a;
  const y = under(v, left + 300) + 52;
  text(ctx, { s: "Six desktop apps", x: left, y, size: 34, color: BODY, a, align: "left" });
  text(ctx, { s: "六个桌面应用", x: left, y: y + 46, size: 28, color: SOFT, a, align: "left", track: 2 });
}

/** App `name`'s letter cubes at `t` in its bar `b`. */
export function nameCubes(name, b, t) {
  const t0 = bar(b);
  const list = word(name, () => INK);
  const build = slots(list, 8);
  const out = slots(list, 4, 0);
  return list.flatMap((c, i) => {
    const at = arrive(t, t0 + (build[i] + 1) * SIX);
    if (!at) return [];
    const gone = lift(t, t0 + (9 + out[i]) * SIX, LAND);
    return [box(c, at.dz + gone.dz, at.a * gone.a)];
  });
}

/** An app's first bar: its name builds at left. */
function app(name, b) {
  return (ctx, t) => {
    const v = place(word(name, () => INK), 16, { left: 112, cy: 800 });
    cubes(ctx, v, nameCubes(name, b, t));
  };
}

/** Bars 41–42: name, version, availability, address. */
function end(ctx, t) {
  const t0 = bar(41);
  const list = word("Ely Office 0.1.0", (g) => (g >= 11 ? ACCENT : LIGHT));
  const name = list.filter((c) => c.glyph < 11);
  const version = list.filter((c) => c.glyph >= 11);
  const slot = new Map([
    ...name.map((c, i) => [c, slots(name, 8)[i]]),
    ...version.map((c, i) => [c, 8 + slots(version, 4)[i]]),
  ]);
  const v = place(list, 21, { cx: 960, cy: 616 });
  const left = bounds(v, list.map((c) => box(c))).x0;
  const boxes = list.flatMap((c) => {
    const at = arrive(t, t0 + (slot.get(c) + 1) * SIX);
    return at ? [box(c, at.dz, at.a)] : [];
  });
  cubes(ctx, v, boxes);
  const fade = (when) => settle(clamp((t - (when - LAND)) / LAND));
  const a = fade(t0 + 3 * BEAT);
  const y = under(v, left + 620) + 56;
  const line = { x: left, align: "left", a };
  text(ctx, { ...line, s: "macOS now · Windows and Linux soon", y, size: 34, color: "#c4c2c0" });
  text(ctx, { ...line, s: "macOS 现已推出 · Windows 与 Linux 即将推出", y: y + 46, size: 28, color: "#acaaa8" });
  const url = "git.zacharyzhang.com/ZacharyZhang-NY/ElyOffice";
  text(ctx, { ...line, s: url, y: y + 122, size: 30, color: "#f5f5f7", a: fade(t0 + BAR), weight: 400 });
}

const frame = (s) => Math.round(s * 30);

/** One range of global frames per bar span. */
function span(name, kind, from, to, draw, first = frame(bar(from))) {
  return { name, kind, first, count: frame(bar(to)) - first, draw };
}

const APPS = ["Doc", "Table", "Show", "PDF", "Tree", "Mark"];

export const SCENES = [
  { name: "type-ely", kind: "base", first: 0, count: 129, draw: ely },
  { name: "type-title", kind: "over", first: 386, count: 129, draw: title },
  ...APPS.map((a, i) => span(`type-${a.toLowerCase()}`, "over", 9 + 4 * i, 10 + 4 * i, app(`Ely${a}`, 9 + 4 * i))),
  { name: "type-end", kind: "over", first: 2572, count: 128, draw: end },
];
