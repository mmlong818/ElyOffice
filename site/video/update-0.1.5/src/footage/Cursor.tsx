import React from 'react';
import { interpolate } from 'remotion';
import { COLOR, EASE } from '../theme';
import { actions, pointerAt, type TakeName } from './take';

const RIPPLE = 0.55;
const RIGHT = '#F2A900';

/** The pointer, drawn from the input log; presses ripple. */
export const Cursor: React.FC<{ take: TakeName; t: number; zoom: number }> = ({ take, t, zoom }) => {
  const p = pointerAt(take, t);
  const downs = actions(take, 'down');
  const ups = actions(take, 'up');
  const held = downs.some((d) => {
    const up = ups.find((u) => u.t >= d.t);
    return t >= d.t && t <= (up ? up.t : d.t) + 0.06;
  });
  // Larger under zoom, but less than the footage grows.
  const size = 44 / Math.sqrt(zoom);
  const press = held ? 0.86 : 1;
  return (
    <>
      {downs
        .filter((d) => t >= d.t && t < d.t + RIPPLE)
        .map((d) => {
          const k = (t - d.t) / RIPPLE;
          const r = interpolate(k, [0, 1], [8, 46], { easing: EASE.out }) / Math.sqrt(zoom);
          const tint = d.right ? RIGHT : COLOR.accent;
          return (
            <div
              key={d.t}
              style={{
                position: 'absolute',
                left: d.x! - r,
                top: d.y! - r,
                width: r * 2,
                height: r * 2,
                borderRadius: '50%',
                border: `${3 / Math.sqrt(zoom)}px solid ${tint}`,
                background: `${tint}22`,
                opacity: 1 - k,
              }}
            />
          );
        })}
      <svg
        width={size}
        height={size * 1.5}
        viewBox="0 0 24 36"
        style={{
          position: 'absolute',
          left: p.x - size * 0.06,
          top: p.y - size * 0.04,
          transformOrigin: '0 0',
          transform: `scale(${press})`,
          filter: 'drop-shadow(0 3px 5px rgba(0,0,0,0.45))',
          overflow: 'visible',
        }}
      >
        <path
          d="M1.5 1.5 L1.5 27 L7.6 21.4 L11.6 30.6 L15.6 28.9 L11.7 19.9 L19.8 19.9 Z"
          fill="#fff"
          stroke="#111"
          strokeWidth={1.6}
          strokeLinejoin="round"
        />
      </svg>
    </>
  );
};
