declare module 'flubber' {
  type Point = [number, number];
  type Shape = string | Point[];
  interface Options {
    maxSegmentLength?: number;
  }
  export function interpolate(from: Shape, to: Shape, options: Options & { string: false }): (t: number) => Point[];
  export function interpolate(from: Shape, to: Shape, options?: Options & { string?: true }): (t: number) => string;
}
