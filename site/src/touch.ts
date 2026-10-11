// Buildings lift on hover or focus and sink when pressed.
import { UNIT } from "./iso.ts";

const SVG = "http://www.w3.org/2000/svg";

/** Links each app's blocks to its region, in reading order. */
export function touch(host: HTMLElement, svg: SVGSVGElement, parts: Map<string, SVGGElement[]>): void {
  host.style.setProperty("--step", `${UNIT}px`);
  const nav = document.createElement("nav");
  nav.className = "doors";
  host.append(nav);

  const owner = new Map([...parts].flatMap(([name, blocks]) => blocks.map((b) => [b, name] as const)));
  const hits = still(svg, owner);

  for (const [name, blocks] of parts) {
    const target = `#${name.toLowerCase()}`;
    const state = { hover: false, focus: false, press: false };
    const hit = hits.filter((h) => h.dataset.part === name);
    const caption = document.createElement("p");
    caption.className = "caption";
    caption.textContent = name;
    caption.setAttribute("aria-hidden", "true");
    host.append(caption);

    const follow = () => place(caption, host, svg, hit);
    const sized = new ResizeObserver(follow);
    const show = () => {
      const up = state.hover || state.focus;
      for (const b of blocks) {
        b.classList.toggle("lifted", up);
        b.classList.toggle("pressed", up && state.press);
      }
      caption.classList.toggle("shown", up);
      // The camera announces each move on the host.
      if (up) {
        host.addEventListener("moved", follow);
        sized.observe(svg);
        follow();
      } else {
        host.removeEventListener("moved", follow);
        sized.disconnect();
      }
    };
    const set = (change: Partial<typeof state>) => {
      Object.assign(state, change);
      show();
    };

    const own = (node: EventTarget | null) => node instanceof SVGElement && node.dataset.part === name;
    for (const h of hit) {
      h.addEventListener("pointerenter", () => set({ hover: true }));
      h.addEventListener("pointerleave", (e) => own(e.relatedTarget) || set({ hover: false, press: false }));
      h.addEventListener("pointerdown", (e) => e.button === 0 && set({ press: true }));
      h.addEventListener("pointerup", () => set({ press: false }));
      h.addEventListener("pointercancel", () => set({ press: false }));
      h.addEventListener("click", () => (location.hash = target));
    }

    const door = document.createElement("a");
    door.href = target;
    door.textContent = name;
    door.addEventListener("focus", () => set({ focus: true }));
    door.addEventListener("blur", () => set({ focus: false, press: false }));
    door.addEventListener("keydown", (e) => e.key === "Enter" && !e.repeat && set({ press: true }));
    door.addEventListener("keyup", (e) => e.key === "Enter" && set({ press: false }));
    nav.append(door);
  }
}

/** Resting outlines over the model, layered as drawn. */
function still(svg: SVGSVGElement, owner: Map<SVGGElement, string>): SVGPolygonElement[] {
  const layer = document.createElementNS(SVG, "g");
  layer.setAttribute("class", "hits");
  const hits: SVGPolygonElement[] = [];
  for (const b of svg.querySelectorAll<SVGGElement>(".block")) {
    const outline = b.querySelector(".outline");
    if (!outline) throw new Error("block has no outline");
    const name = owner.get(b);
    b.dataset.part = name ?? "";
    const face = outline.cloneNode() as SVGPolygonElement;
    face.setAttribute("class", name ? "hit-face" : "hit-face ground");
    face.dataset.part = name ?? "";
    layer.append(face);
    if (name) hits.push(face);
  }
  svg.append(layer);
  return hits;
}

/** Puts the caption one cell above the lifted top. */
function place(caption: HTMLElement, host: HTMLElement, svg: SVGSVGElement, faces: SVGPolygonElement[]) {
  const origin = host.getBoundingClientRect();
  const ctm = svg.getScreenCTM();
  if (!ctm) throw new Error("model is not rendered");
  // Hit faces never lift, so they give the rest pose.
  const rects = faces.map((f) => f.getBoundingClientRect());
  const left = Math.min(...rects.map((r) => r.left));
  const right = Math.max(...rects.map((r) => r.right));
  const top = Math.min(...rects.map((r) => r.top));
  caption.style.left = `${(left + right) / 2 - origin.left}px`;
  caption.style.top = `${top - origin.top - 2 * UNIT * ctm.d}px`;
}
