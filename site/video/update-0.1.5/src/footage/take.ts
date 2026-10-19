import cells from '../../public/footage/cells.json';
import doc from '../../public/footage/doc.json';
import menus from '../../public/footage/menus.json';
import more from '../../public/footage/more.json';
import show from '../../public/footage/show.json';
import table from '../../public/footage/table.json';

// Times are footage seconds; positions are footage pixels (2048×1152).
export type Event = {
  t: number;
  kind: string;
  x?: number;
  y?: number;
  right?: boolean;
  key?: string;
  ch?: string;
  down?: boolean;
};

export type Box = [number, number, number, number];

export type Take = {
  fps: number;
  size: number[];
  events: Event[];
  menus: { parent: number[]; panel: number[] } | null;
};

export const TAKES = { menus, cells, doc, table, show, more } as Record<string, Take>;
export type TakeName = 'menus' | 'cells' | 'doc' | 'table' | 'show' | 'more';

const placed = new Map<string, Event[]>();
const pointed = (name: TakeName) => {
  let list = placed.get(name);
  if (!list) {
    list = TAKES[name].events.filter((e) => e.x !== undefined && e.kind !== 'sync');
    placed.set(name, list);
  }
  return list;
};

/** Where the pointer was at footage time `t`. */
export const pointerAt = (name: TakeName, t: number) => {
  const list = pointed(name);
  let lo = 0;
  let hi = list.length - 1;
  if (t <= list[0].t) return { x: list[0].x!, y: list[0].y! };
  if (t >= list[hi].t) return { x: list[hi].x!, y: list[hi].y! };
  while (hi - lo > 1) {
    const mid = (lo + hi) >> 1;
    if (list[mid].t <= t) lo = mid;
    else hi = mid;
  }
  const a = list[lo];
  const b = list[hi];
  const f = b.t === a.t ? 1 : (t - a.t) / (b.t - a.t);
  return { x: a.x! + (b.x! - a.x!) * f, y: a.y! + (b.y! - a.y!) * f };
};

/** Events of `kinds` in the take, in order. */
export const actions = (name: TakeName, ...kinds: string[]) =>
  TAKES[name].events.filter((e) => kinds.includes(e.kind));
