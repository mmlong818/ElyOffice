// Montage: six screens on the turning model, in WebGL.
import { model } from "../../src/model.ts";
import { type Box, screens } from "../scenes/parts.ts";
import { slabs } from "../scenes/turn.ts";
import { BEAT, glide, toward } from "../scenes/time.ts";
import recs from "./recs.json";

const WIDTH = 1920;
const HEIGHT = 1080;
const FPS = 30;
/** Bar 33 downbeat to the frame before bar 35. */
export const FIRST = recs.first;
export const COUNT = 129;
const TURN: [number, number] = [128, 136];
const PIVOT = [8, 5.5];
const apps = recs.apps;

const ground = model.flatMap((p) => p.blocks).filter((b) => b !== screens.ElyShow);

type Rgb = [number, number, number];

/** sRGB hex to linear channels. */
function linear(hex: string): Rgb {
  const n = parseInt(hex.slice(1), 16);
  return [16, 8, 0].map((s) => {
    const c = ((n >> s) & 255) / 255;
    return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  }) as Rgb;
}

const encode = (c: number) => (c <= 0.0031308 ? 12.92 * c : 1.055 * c ** (1 / 2.4) - 0.055);

/** Mixes with black in oklab, like CSS color-mix. */
function shade(hex: string, black: number): Rgb {
  const [r, g, b] = linear(hex);
  const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
  const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
  const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
  const k = 1 - black;
  const [L, A, B] = [
    k * (0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s),
    k * (1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s),
    k * (0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s),
  ];
  const l3 = (L + 0.3963377774 * A + 0.2158037573 * B) ** 3;
  const m3 = (L - 0.1055613458 * A - 0.0638541728 * B) ** 3;
  const s3 = (L - 0.0894841775 * A - 1.291485548 * B) ** 3;
  return [
    4.0767416621 * l3 - 3.3077115913 * m3 + 0.2309699292 * s3,
    -1.2684380046 * l3 + 2.6097574011 * m3 - 0.3413193965 * s3,
    -0.0041960863 * l3 - 0.7034186147 * m3 + 1.707614701 * s3,
  ].map((c) => encode(Math.min(1, Math.max(0, c)))) as Rgb;
}

/** Side darkness: +y faces 9%, +x faces 20%, light fixed. */
function darkness(nx: number, ny: number): number {
  const u = Math.atan2(ny, nx) - Math.PI / 2;
  return 0.145 - 0.055 * (Math.cos(u) + Math.sin(u));
}

const COS = Math.cos(Math.PI / 6);

/** World point turned by `a`, to iso units and depth. */
function iso(a: number, x: number, y: number, z: number): [number, number, number] {
  const [dx, dy] = [x - PIVOT[0], y - PIVOT[1]];
  const [rx, ry] = [dx * Math.cos(a) - dy * Math.sin(a), dx * Math.sin(a) + dy * Math.cos(a)];
  return [(rx - ry) * COS, (rx + ry) / 2 - z, rx + ry + z];
}

const corners = (b: Box) =>
  [0, 1].flatMap((i) => [0, 1].flatMap((j) => [0, 1].map((k) => [b.x + i * b.w, b.y + j * b.d, b.z + k * b.h])));

/** Fixed scale and offset fitting every angle of the turn. */
const camera = (() => {
  const pts = [...ground, ...slabs, ...slabs.map((b) => ({ ...b, z: b.z + 1 }))].flatMap(corners);
  const xs: number[] = [];
  const ys: number[] = [];
  for (let i = 0; i <= 90; i++) {
    for (const [x, y, z] of pts) {
      const [px, py] = iso((-i * Math.PI) / 180, x, y, z);
      xs.push(px);
      ys.push(py);
    }
  }
  const [x0, x1, y0, y1] = [Math.min(...xs), Math.max(...xs), Math.min(...ys), Math.max(...ys)];
  const s = Math.min(1800 / (x1 - x0), 1000 / (y1 - y0));
  return { s, x: WIDTH / 2 - (s * (x0 + x1)) / 2, y: HEIGHT / 2 - (s * (y0 + y1)) / 2 };
})();

/** Frame pixels to clip space; nearer is smaller depth. */
function clip(a: number, x: number, y: number, z: number): [number, number, number] {
  const [px, py, k] = iso(a, x, y, z);
  return [((camera.x + camera.s * px) / WIDTH) * 2 - 1, 1 - ((camera.y + camera.s * py) / HEIGHT) * 2, -k / 64];
}

type V = [number, number, number];
/** Box faces except the bottom, as corner lists with normals. */
function faces(b: Box): { n: V; c: V[] }[] {
  const { x, y, z, w, d, h } = b;
  const [X, Y, Z] = [x + w, y + d, z + h];
  return [
    { n: [0, 0, 1], c: [[x, y, Z], [X, y, Z], [X, Y, Z], [x, Y, Z]] },
    { n: [1, 0, 0], c: [[X, y, Z], [X, Y, Z], [X, Y, z], [X, y, z]] },
    { n: [-1, 0, 0], c: [[x, Y, Z], [x, y, Z], [x, y, z], [x, Y, z]] },
    { n: [0, 1, 0], c: [[X, Y, Z], [x, Y, Z], [x, Y, z], [X, Y, z]] },
    { n: [0, -1, 0], c: [[x, y, Z], [X, y, Z], [X, y, z], [x, y, z]] },
  ];
}

const canvas = document.querySelector<HTMLCanvasElement>("#frame")!;
const gl = canvas.getContext("webgl2", { alpha: false, antialias: true, preserveDrawingBuffer: true })!;
if (!gl) throw new Error("no WebGL2");
const aniso = gl.getExtension("EXT_texture_filter_anisotropic");

function program(vs: string, fs: string): WebGLProgram {
  const p = gl.createProgram()!;
  for (const [type, src] of [[gl.VERTEX_SHADER, vs], [gl.FRAGMENT_SHADER, fs]] as const) {
    const sh = gl.createShader(type)!;
    gl.shaderSource(sh, src);
    gl.compileShader(sh);
    if (!gl.getShaderParameter(sh, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(sh) ?? "shader");
    gl.attachShader(p, sh);
  }
  gl.bindAttribLocation(p, 0, "p");
  gl.bindAttribLocation(p, 1, "v");
  gl.linkProgram(p);
  if (!gl.getProgramParameter(p, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(p) ?? "link");
  return p;
}

const HEAD = "#version 300 es\nprecision highp float;\nlayout(location=0) in vec3 p;layout(location=1) in vec4 v;";
const flat = program(
  `${HEAD}out vec4 c;void main(){c=v;gl_Position=vec4(p,1.);}`,
  "#version 300 es\nprecision highp float;in vec4 c;out vec4 o;void main(){o=c;}",
);
const film = program(
  `${HEAD}out vec2 u;out float q;void main(){u=v.xy;q=v.z;gl_Position=vec4(p,1.);}`,
  "#version 300 es\nprecision highp float;uniform sampler2D t;in vec2 u;in float q;out vec4 o;void main(){o=vec4(texture(t,u).rgb,q);}",
);
const buffer = gl.createBuffer();
const textures = apps.map(() => gl.createTexture()!);

/** Draws quads as triangles: 7 floats per vertex. */
function draw(prog: WebGLProgram, data: number[]) {
  gl.useProgram(prog);
  gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
  gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(data), gl.STREAM_DRAW);
  gl.enableVertexAttribArray(0);
  gl.enableVertexAttribArray(1);
  gl.vertexAttribPointer(0, 3, gl.FLOAT, false, 28, 0);
  gl.vertexAttribPointer(1, 4, gl.FLOAT, false, 28, 12);
  gl.drawArrays(gl.TRIANGLES, 0, data.length / 7);
}

/** Two triangles from four clip points and per-corner values. */
function quad(out: number[], pts: V[], vals: number[][]) {
  for (const i of [0, 1, 2, 0, 2, 3]) out.push(...pts[i], ...vals[i]);
}

/** Pushes a box's faces; `skip` leaves the screen face. */
function solid(out: number[], a: number, b: Box, alpha: number, skip: boolean) {
  for (const { n, c } of faces(b)) {
    if (skip && n[1] === 1) continue;
    const [nx, ny] = [n[0] * Math.cos(a) - n[1] * Math.sin(a), n[0] * Math.sin(a) + n[1] * Math.cos(a)];
    if (n[2] === 0 && nx + ny <= 1e-9) continue;
    const rgb = shade(b.tone, n[2] ? 0 : darkness(nx, ny));
    quad(out, c.map(([x, y, z]) => clip(a, x, y, z)), c.map(() => [...rgb, alpha]));
  }
}

async function load(i: number, url: string) {
  const img = new Image();
  img.src = url;
  await img.decode();
  gl.bindTexture(gl.TEXTURE_2D, textures[i]);
  gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, img);
  gl.generateMipmap(gl.TEXTURE_2D);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR_MIPMAP_LINEAR);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
  if (aniso) gl.texParameterf(gl.TEXTURE_2D, aniso.TEXTURE_MAX_ANISOTROPY_EXT, 8);
}

/** Renders global frame `f`; returns each visible screen's corners. */
async function render(f: number) {
  if (f < FIRST || f >= FIRST + COUNT) throw new Error(`frame ${f} is outside the montage`);
  const t = f / FPS;
  const a = (-Math.PI / 2) * glide((t - TURN[0] * BEAT) / ((TURN[1] - TURN[0]) * BEAT));
  await Promise.all(apps.map((s, i) => load(i, `rec/${s.rec}/${String(f - FIRST + 1).padStart(5, "0")}.jpg`)));
  gl.viewport(0, 0, canvas.width, canvas.height);
  gl.clearColor(...shade("#fbfbfa", 0), 1);
  gl.depthMask(true);
  gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
  gl.enable(gl.DEPTH_TEST);
  gl.depthFunc(gl.LEQUAL);
  gl.disable(gl.BLEND);
  const opaque: number[] = [];
  for (const b of ground) solid(opaque, a, b, 1, false);
  const shown = apps.map((s, i) => ({ s, i, land: t > s.beat * BEAT - 0.2 ? toward(t, s.beat) : -1 }));
  const standing = shown.filter((x) => x.land >= 1);
  const landing = shown.filter((x) => x.land >= 0 && x.land < 1);
  for (const { i } of standing) solid(opaque, a, slabs[i], 1, true);
  draw(flat, opaque);
  const faceOf = (i: number, lift: number, alpha: number) => {
    const b = slabs[i];
    const [x, X, Y, z, Z] = [b.x, b.x + b.w, b.y + b.d, b.z + lift, b.z + b.h + lift];
    const c: V[] = [[x, Y, Z], [X, Y, Z], [X, Y, z], [x, Y, z]];
    const data: number[] = [];
    quad(data, c.map(([px, py, pz]) => clip(a, px, py, pz)), [[0, 0, alpha, 0], [1, 0, alpha, 0], [1, 1, alpha, 0], [0, 1, alpha, 0]]);
    gl.bindTexture(gl.TEXTURE_2D, textures[i]);
    draw(film, data);
    return c.map(([px, py, pz]) => {
      const [ix, iy] = iso(a, px, py, pz);
      return [camera.x + camera.s * ix, camera.y + camera.s * iy];
    });
  };
  const pins: Record<string, number[][]> = {};
  for (const { s, i } of standing) pins[s.app] = faceOf(i, 0, 1);
  gl.enable(gl.BLEND);
  gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);
  gl.depthMask(false);
  for (const { i, land } of landing) {
    const sides: number[] = [];
    solid(sides, a, { ...slabs[i], z: slabs[i].z + 1 - land }, land, true);
    draw(flat, sides);
    faceOf(i, 1 - land, land);
  }
  gl.finish();
  await new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(done)));
  return { angle: (a * 180) / Math.PI, pins };
}

declare global {
  interface Window {
    montage: { FIRST: number; COUNT: number; apps: typeof apps; render: typeof render };
  }
}
window.montage = { FIRST, COUNT, apps, render };
