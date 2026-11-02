// The desk model: seven apps, each its own object.
import type { Block } from "./iso.ts";

const PAPER = "#f4f4f2";
const DESK = "#e6e6e2";
/** The one accent: roads and the selected cell. */
export const ACCENT = "#3d7fe5";
const TOP = 0.6;

/** One part of the model, grown together. */
export interface Part {
  name: string;
  blocks: Required<Block>[];
}

const on = (x: number, y: number, w: number, d: number, h: number, z = 0, tone = PAPER): Required<Block> => ({
  x, y, z: TOP + z, w, d, h, tone,
});

/** A stack of pages. */
const doc = [0, 1, 2, 3].map((i) => on(1 + i * 0.08, 1 - i * 0.08, 2.6, 3, 0.25, i * 0.25));

/** A three by three sheet; one cell is selected. */
const heights = [1.2, 2.4, 1.6, 0.8, 1.8, 1, 0.5, 1.4, 0.7];
const table = heights.map((h, i) =>
  on(5 + (i % 3), 1 + Math.floor(i / 3), 0.9, 0.9, h, 0, i === 4 ? ACCENT : PAPER),
);

/** A slide on its stand. */
const show = [on(9.4, 1, 3.2, 0.25, 2.3), on(9.4, 1.25, 3.2, 1.6, 0.25)];

/** A page with its corner folded down. */
const pdf = [on(13, 3.2, 2.4, 2.4, 1), on(13, 5.6, 1.6, 0.8, 1), on(14.6, 5.6, 0.8, 0.8, 0.3)];

/** A mailbox on its post, flag up. */
const mail = [on(11.85, 9.2, 0.3, 0.3, 1.4), on(11.5, 8.7, 1, 1.5, 0.8, 1.4), on(12.5, 9, 0.08, 0.3, 0.8, 1.8, ACCENT)];

/** A trunk and a branch, joined by a merge. */
const tree = [on(6.5, 7.6, 0.7, 0.7, 2.8), on(8.7, 7.6, 0.7, 0.7, 1.8), on(7.2, 7.6, 1.5, 0.7, 0.3, 1.2)];

/** Headings stepping down. */
const mark = [0, 1, 2].map((i) => on(1, 7 + i, 3, 1, 2 - i * 0.6));

/** Roads: exports go to PDF; Markdown goes to Git. */
const road = (x: number, y: number, w: number, d: number) => on(x, y, w, d, 0.08, 0, ACCENT);
const roads = [
  road(2, 4, 0.6, 0.6),
  road(6, 3.9, 0.6, 0.7),
  road(10.7, 2.85, 0.6, 1.75),
  road(2, 4.6, 11, 0.6),
  road(2, 5.2, 0.6, 1.8),
  road(4, 7.6, 2.5, 0.6),
];

/** Growth order: desk, the seven apps, then roads. */
export const model: Part[] = [
  { name: "Desk", blocks: [{ x: 0, y: 0, z: 0, w: 16, d: 11, h: TOP, tone: DESK }] },
  { name: "ElyDoc", blocks: doc },
  { name: "ElyTable", blocks: table },
  { name: "ElyShow", blocks: show },
  { name: "ElyPDF", blocks: pdf },
  { name: "ElyMail", blocks: mail },
  { name: "ElyTree", blocks: tree },
  { name: "ElyMark", blocks: mark },
  { name: "Roads", blocks: roads },
];
