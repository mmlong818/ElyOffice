// Roads: drag a file from its app to its target.
import "./play.css";
import { block, order, project, stage, UNIT, type Block } from "./iso.ts";
import { ACCENT, model } from "./model.ts";
import { said, TEXT, type Key } from "./text.ts";

type Cell = [number, number];

const BEAT = 60 / 112;
const STEP = BEAT / 4;
const TOP = 0.6;
const RISE = 0.1;
const FILE = 0.56;
const THICK = 0.36;
const DESK = "#e6e6e2";
const PAPER = "#f4f4f2";
const ROAD = "#d3d3ce";
const ROAD_NIGHT = "#474745";
const REACH = 0.9;

/** Each building's back corner on the board. */
const HOMES: Record<string, Cell> = {
  ElyDoc: [1, 1],
  ElyTable: [5, 1],
  ElyShow: [9, 1],
  ElyPDF: [14, 1],
  ElyMail: [9, 9],
  ElyMark: [1, 9],
  ElyTree: [13, 11],
};

/** Cells along straight runs between corners. */
function walk(...corners: Cell[]): Cell[] {
  const out: Cell[] = [corners[0]];
  for (const [x, y] of corners.slice(1)) {
    let [cx, cy] = out[out.length - 1];
    if (cx !== x && cy !== y) throw new Error("roads turn at right angles");
    while (cx !== x || cy !== y) {
      cx += Math.sign(x - cx);
      cy += Math.sign(y - cy);
      out.push([cx, cy]);
    }
  }
  return out;
}

interface Link {
  from: string;
  to: string;
  line: Key;
  path: Cell[];
}

const LINKS: Link[] = [
  { from: "ElyDoc", to: "ElyPDF", line: "toPdf", path: walk([2, 5], [2, 6], [15, 6], [15, 5]) },
  { from: "ElyTable", to: "ElyPDF", line: "toPdf", path: walk([6, 5], [6, 6], [15, 6], [15, 5]) },
  { from: "ElyShow", to: "ElyPDF", line: "toPdf", path: walk([10, 4], [10, 6], [15, 6], [15, 5]) },
  { from: "ElyMark", to: "ElyPDF", line: "toPdf", path: walk([5, 10], [5, 6], [15, 6], [15, 5]) },
  { from: "ElyMark", to: "ElyTree", line: "toTree", path: walk([5, 10], [5, 12], [14, 12]) },
];

const SOURCES = ["ElyDoc", "ElyTable", "ElyShow", "ElyMark"] as const;
const NAMES: Record<(typeof SOURCES)[number], Key> = {
  ElyDoc: "fileDoc",
  ElyTable: "fileTable",
  ElyShow: "fileShow",
  ElyMark: "fileMark",
};

/** Arrows as screen directions; ties take this grid step. */
const ARROWS: Record<string, [number, number, Cell]> = {
  ArrowRight: [1, 0, [1, 0]],
  ArrowLeft: [-1, 0, [-1, 0]],
  ArrowUp: [0, -1, [0, -1]],
  ArrowDown: [0, 1, [0, 1]],
};

const id = (c: Cell) => `${c[0]},${c[1]}`;
const same = (a: Cell, b: Cell) => a[0] === b[0] && a[1] === b[1];
const still = () => matchMedia("(prefers-reduced-motion: reduce)").matches;
const lang = () => (document.documentElement.lang === "zh-CN" ? "zh" : "en");

interface File {
  name: string;
  start: Cell;
  cell: Cell;
  goal: Cell;
  level: number;
  done: boolean;
  wrap: SVGGElement;
  links: Link[];
  ways: Set<string>;
}

interface Item {
  el: SVGGElement;
  box: () => Required<Block>;
}

/** A building moved home; accent is kept for files. */
function placed(name: string): Required<Block>[] {
  const part = model.find((p) => p.name === name);
  if (!part) throw new Error(`no building ${name}`);
  const [x, y] = HOMES[name];
  const left = Math.min(...part.blocks.map((b) => b.x));
  const back = Math.min(...part.blocks.map((b) => b.y));
  const floor = Math.min(...part.blocks.map((b) => b.z));
  const snap = (v: number) => Math.round(v * 1000) / 1000;
  return part.blocks.map((b) => ({ ...b, tone: PAPER, x: snap(b.x - left + x), y: snap(b.y - back + y), z: snap(b.z - floor + TOP) }));
}

const fileBox = (f: File): Required<Block> => ({
  x: f.cell[0] + (1 - FILE) / 2,
  y: f.cell[1] + (1 - FILE) / 2,
  z: TOP + RISE + f.level * THICK,
  w: FILE,
  d: FILE,
  h: THICK,
  tone: ACCENT,
});

/** The roads section: board, files, and one line. */
export function playground(): HTMLElement {
  const section = document.createElement("section");
  section.id = "roads";
  section.className = "play";
  const names = document.createElement("div");
  names.hidden = true;
  const hint = said("span", "moveHint");
  hint.id = "roads-hint";
  names.append(hint);

  const items: Item[] = [];
  const add = (spec: Required<Block>) => {
    const g = block(spec);
    items.push({ el: g, box: () => spec });
    return g;
  };
  const desk = add({ x: 0, y: 0, z: 0, w: 17.5, d: 13.5, h: TOP, tone: DESK });
  const buildings = new Map(Object.keys(HOMES).map((n) => [n, placed(n).map(add)]));
  const roads = new Map<string, SVGGElement>();
  for (const cell of LINKS.flatMap((l) => l.path)) {
    if (roads.has(id(cell))) continue;
    const g = add({ x: cell[0] + 0.05, y: cell[1] + 0.05, z: TOP, w: 0.9, d: 0.9, h: RISE, tone: ROAD });
    paint(g, ROAD, ROAD_NIGHT);
    g.dataset.cell = id(cell);
    roads.set(id(cell), g);
  }

  const files: File[] = SOURCES.map((name) => {
    const links = LINKS.filter((l) => l.from === name);
    const start = links[0].path[0];
    const wrap = document.createElementNS("http://www.w3.org/2000/svg", "g");
    wrap.setAttribute("class", "file");
    wrap.setAttribute("tabindex", "0");
    wrap.setAttribute("role", "button");
    const label = said("span", NAMES[name]);
    label.id = `roads-${name}`;
    names.append(label);
    wrap.setAttribute("aria-labelledby", label.id);
    wrap.setAttribute("aria-describedby", hint.id);
    const f: File = {
      name, start, cell: start, goal: start, level: 0, done: false, wrap, links,
      ways: new Set(links.flatMap((l) => l.path.map(id))),
    };
    items.push({ el: wrap, box: () => fileBox(f) });
    return f;
  });
  files.forEach(render);

  const svg = stage(items.flatMap((i) => (i.el.classList.contains("file") ? [] : [i.el])));
  svg.classList.add("grow");
  svg.style.setProperty("--step", `${UNIT}px`);
  const total = timed(desk, buildings, roads, files);

  const board = document.createElement("div");
  board.className = "board";
  board.append(svg, ...captions(svg, buildings));

  const news = document.createElement("p");
  news.className = "news";
  news.setAttribute("aria-live", "polite");
  const again = said("button", "putBack");
  again.className = "again";
  again.type = "button";
  again.hidden = true;
  const foot = document.createElement("div");
  foot.className = "foot";
  foot.append(news, again);
  section.append(said("h2", "roads"), board, foot, names);

  let ready = false;
  const timers = new Set<number>();
  const later = (ms: number, run: () => void) => {
    const t = window.setTimeout(() => {
      timers.delete(t);
      run();
    }, ms);
    timers.add(t);
  };
  const grown = () => {
    svg.classList.remove("grow");
    ready = true;
  };
  if (still()) grown();
  const seen = new IntersectionObserver((entries) => {
    if (!entries.some((e) => e.isIntersecting)) return;
    seen.disconnect();
    svg.classList.add("seen");
    if (!ready) window.setTimeout(grown, total * 1000 + 250);
  }, { rootMargin: "0px 0px -20% 0px" });
  seen.observe(section);

  /** Painter order; `fixed` keeps its DOM place and focus. */
  const arrange = (fixed: SVGGElement | null) => {
    const els = order(items.map((i) => i.box())).map((i) => items[i].el);
    const at = fixed ? els.indexOf(fixed) : -1;
    if (!fixed || at < 0) return svg.append(...els);
    for (const el of els.slice(0, at)) svg.insertBefore(el, fixed);
    svg.append(...els.slice(at + 1));
  };
  arrange(null);

  let walking: SVGGElement | null = null;
  const step = (f: File, next: Cell) => {
    for (const o of files) {
      if (o === f || !same(o.cell, f.cell) || o.level < f.level) continue;
      o.level -= 1;
      render(o);
    }
    f.level = files.filter((o) => o !== f && same(o.cell, next)).length;
    f.cell = next;
    render(f);
    arrange(f.wrap);
    again.hidden = false;
    if (!still()) {
      f.wrap.animate([{ translate: `0 -${UNIT}px` }, { translate: "0 0" }], {
        duration: STEP * 1000,
        easing: "cubic-bezier(0.2, 0.8, 0.3, 1)",
      });
      click();
    }
    const link = f.links.find((l) => same(l.path[l.path.length - 1], next));
    if (link) arrive(f, link);
  };

  const tick = (f: File) => {
    walking = null;
    while (!f.done && !same(f.cell, f.goal)) {
      step(f, route(f, f.goal)[1]);
      if (!still() && !f.done && !same(f.cell, f.goal)) {
        walking = f.wrap;
        return later(STEP * 1000, () => tick(f));
      }
    }
  };
  const go = (f: File, goal: Cell) => {
    if (f.done || same(goal, f.goal)) return;
    f.goal = goal;
    if (walking !== f.wrap) tick(f);
  };

  const arrive = (f: File, link: Link) => {
    f.done = true;
    f.goal = f.cell;
    news.dataset.t = link.line;
    news.textContent = TEXT[lang()][link.line];
    link.path.forEach((cell, i) => {
      const g = roads.get(id(cell))!;
      const light = () => paint(g, ACCENT, ACCENT);
      if (still()) light();
      else later((i + 1) * STEP * 1000, light);
    });
    if (still()) return;
    const house = buildings.get(link.to)!;
    for (const b of house) b.classList.add("lifted");
    later(BEAT * 1000, () => house.forEach((b) => b.classList.remove("lifted")));
  };

  again.addEventListener("click", () => {
    for (const t of timers) clearTimeout(t);
    timers.clear();
    walking = null;
    for (const f of files) Object.assign(f, { cell: f.start, goal: f.start, level: 0, done: false });
    files.forEach(render);
    for (const g of roads.values()) paint(g, ROAD, ROAD_NIGHT);
    for (const b of buildings.values()) b.forEach((g) => g.classList.remove("lifted"));
    delete news.dataset.t;
    news.textContent = "";
    arrange(null);
    files[files.length - 1].wrap.focus();
    again.hidden = true;
  });

  let drag: { f: File; pointer: number; dx: number; dy: number } | null = null;
  for (const f of files) {
    f.wrap.addEventListener("pointerdown", (e) => {
      if (!ready || f.done || e.button !== 0) return;
      e.preventDefault();
      f.wrap.focus({ focusVisible: false });
      wake();
      svg.setPointerCapture(e.pointerId);
      const [gx, gy] = grid(svg, e);
      drag = { f, pointer: e.pointerId, dx: gx - f.cell[0] - 0.5, dy: gy - f.cell[1] - 0.5 };
    });
    f.wrap.addEventListener("keydown", (e) => {
      const arrow = ARROWS[e.key];
      if (!arrow) return;
      e.preventDefault();
      if (!ready || f.done) return;
      wake();
      const next = toward(f, f.goal, arrow);
      if (next) go(f, next);
    });
  }
  svg.addEventListener("pointermove", (e) => {
    if (!drag || e.pointerId !== drag.pointer) return;
    const [gx, gy] = grid(svg, e);
    const near = nearest(drag.f, gx - drag.dx, gy - drag.dy);
    if (near) go(drag.f, near);
  });
  const drop = (e: PointerEvent) => {
    if (drag && e.pointerId === drag.pointer) drag = null;
  };
  svg.addEventListener("pointerup", drop);
  svg.addEventListener("pointercancel", drop);
  return section;
}

/** Sets a road cell's day and night tones. */
function paint(g: SVGGElement, day: string, night: string): void {
  g.style.setProperty("--tone", day);
  g.style.setProperty("--tone-dark", night);
}

/** Rebuilds the file's block at its cell and level. */
function render(f: File): void {
  f.wrap.replaceChildren(block(fileBox(f)));
  f.wrap.dataset.cell = id(f.cell);
}

/** Landing times: desk, buildings, roads in waves, then files. */
function timed(desk: SVGGElement, buildings: Map<string, SVGGElement[]>, roads: Map<string, SVGGElement>, files: File[]): number {
  let at = BEAT;
  const land = (gs: SVGGElement[]) => {
    for (const g of gs) g.style.setProperty("--land", `${at.toFixed(3)}s`);
    at += STEP;
  };
  land([desk]);
  for (const house of buildings.values()) land(house);
  const waves = new Map<number, SVGGElement[]>();
  for (const [key, g] of roads) {
    const sum = key.split(",").map(Number).reduce((a, b) => a + b);
    waves.set(sum, [...(waves.get(sum) ?? []), g]);
  }
  for (const sum of [...waves.keys()].sort((a, b) => a - b)) land(waves.get(sum)!);
  land(files.map((f) => f.wrap));
  return at;
}

/** Building names above their tops, placed in viewBox percent. */
function captions(svg: SVGSVGElement, buildings: Map<string, SVGGElement[]>): HTMLElement[] {
  const view = svg.viewBox.baseVal;
  return [...buildings.keys()].map((name) => {
    const blocks = placed(name);
    const xs = blocks.flatMap((b) => [b.x, b.x + b.w]);
    const ys = blocks.flatMap((b) => [b.y, b.y + b.d]);
    const tops = blocks.flatMap((b) => [project(b.x, b.y, b.z + b.h)[1], project(b.x + b.w, b.y, b.z + b.h)[1]]);
    const [cx] = project((Math.min(...xs) + Math.max(...xs)) / 2, (Math.min(...ys) + Math.max(...ys)) / 2, 0);
    const p = document.createElement("p");
    p.className = "caption";
    p.textContent = name;
    p.setAttribute("aria-hidden", "true");
    p.style.left = `${((cx - view.x) / view.width) * 100}%`;
    p.style.top = `${((Math.min(...tops) - UNIT * 0.5 - view.y) / view.height) * 100}%`;
    return p;
  });
}

/** The pointer on the road plane, in grid units. */
function grid(svg: SVGSVGElement, e: PointerEvent): [number, number] {
  const m = svg.getScreenCTM();
  if (!m) throw new Error("board is not rendered");
  const p = new DOMPoint(e.clientX, e.clientY).matrixTransform(m.inverse());
  const [cx, sy] = project(1, 0, 0);
  const a = p.x / cx;
  const b = (p.y + (TOP + RISE) * UNIT) / sy;
  return [(a + b) / 2, (b - a) / 2];
}

/** Nearest road cell to a grid point, if close. */
function nearest(f: File, x: number, y: number): Cell | null {
  let best: Cell | null = null;
  let far = REACH;
  for (const key of f.ways) {
    const cell = key.split(",").map(Number) as Cell;
    const d = Math.hypot(cell[0] + 0.5 - x, cell[1] + 0.5 - y);
    if (d < far) [best, far] = [cell, d];
  }
  return best;
}

/** Shortest road from the file to `goal`, ends included. */
function route(f: File, goal: Cell): Cell[] {
  const back = new Map<string, Cell | null>([[id(f.cell), null]]);
  const queue: Cell[] = [f.cell];
  while (queue.length) {
    const at = queue.shift()!;
    if (same(at, goal)) {
      const path: Cell[] = [];
      for (let c: Cell | null = at; c; c = back.get(id(c))!) path.unshift(c);
      return path;
    }
    for (const n of around(f, at)) {
      if (back.has(id(n))) continue;
      back.set(id(n), at);
      queue.push(n);
    }
  }
  throw new Error(`no road from ${id(f.cell)} to ${id(goal)}`);
}

const around = (f: File, [x, y]: Cell): Cell[] =>
  ([[x + 1, y], [x - 1, y], [x, y + 1], [x, y - 1]] as Cell[]).filter((c) => f.ways.has(id(c)));

/** Road cell beside `from` that best follows an arrow. */
function toward(f: File, from: Cell, [ax, ay, prefer]: [number, number, Cell]): Cell | null {
  let best: Cell | null = null;
  let score = 0.1;
  for (const n of around(f, from)) {
    const step: Cell = [n[0] - from[0], n[1] - from[1]];
    const [sx, sy] = project(step[0], step[1], 0);
    const s = (sx * ax + sy * ay) / Math.hypot(sx, sy) + (same(step, prefer) ? 0.01 : 0);
    if (s > score) [best, score] = [n, s];
  }
  return best;
}

let audio: AudioContext | null = null;

/** Starts sound only after the reader touches a file. */
function wake(): void {
  if (!audio && !still()) audio = new AudioContext();
}

/** A short tick for each landing. */
function click(): void {
  if (!audio) return;
  const t = audio.currentTime;
  const tone = audio.createOscillator();
  const gain = audio.createGain();
  tone.type = "triangle";
  tone.frequency.value = 1320;
  gain.gain.setValueAtTime(0.04, t);
  gain.gain.exponentialRampToValueAtTime(0.0001, t + 0.04);
  tone.connect(gain).connect(audio.destination);
  tone.start(t);
  tone.stop(t + 0.05);
}
