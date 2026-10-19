import { cancelRender, continueRender, delayRender, interpolate, staticFile, useCurrentFrame } from 'remotion';
import { COLOR, EASE, FONT } from '../theme';

// The 'before' card mixes faces, as Windows did.
const serif = delayRender('Loading Noto Serif SC', { timeoutInMilliseconds: 120000 });
new FontFace('Noto Serif SC', `url(${staticFile('fonts/NotoSerifSC.ttf')}) format("truetype")`, { weight: '200 900' })
  .load()
  .then((face) => {
    document.fonts.add(face);
    continueRender(serif);
  }, cancelRender);

const clamp = { extrapolateLeft: 'clamp', extrapolateRight: 'clamp' } as const;
const BEAT = 37.5;

const Card: React.FC<{ at: number; x: number; title: string; sub: string; children: React.ReactNode }> = ({ at, x, title, sub, children }) => {
  const f = useCurrentFrame();
  const p = interpolate(f, [at, at + 34], [0, 1], { ...clamp, easing: EASE.out });
  return (
    <div
      style={{
        position: 'absolute',
        left: x,
        top: 150,
        width: 700,
        height: 640,
        borderRadius: 28,
        background: 'linear-gradient(180deg, rgba(255,255,255,0.055), rgba(255,255,255,0.02))',
        border: `1px solid ${COLOR.hairline}`,
        boxShadow: '0 30px 80px rgba(0,0,0,0.35)',
        opacity: p,
        transform: `translateY(${(1 - p) * 40}px) scale(${0.96 + 0.04 * p})`,
        overflow: 'hidden',
      }}
    >
      <div style={{ position: 'absolute', inset: '0 0 190px 0' }}>{children}</div>
      <div style={{ position: 'absolute', left: 48, right: 48, bottom: 56 }}>
        <div style={{ fontFamily: FONT.sans, fontSize: 40, fontWeight: 600, color: COLOR.ink, letterSpacing: '0.02em' }}>{title}</div>
        <div style={{ fontFamily: FONT.sans, fontSize: 24, fontWeight: 400, color: COLOR.muted, marginTop: 12, lineHeight: 1.5 }}>{sub}</div>
      </div>
    </div>
  );
};

/** One word in two faces, settling into one. */
const Glyphs: React.FC<{ at: number }> = ({ at }) => {
  const f = useCurrentFrame();
  const fix = interpolate(f, [at + 2 * BEAT, at + 3.4 * BEAT], [0, 1], { ...clamp, easing: EASE.inOut });
  const label = (text: string, on: number) => (
    <span style={{ fontFamily: FONT.sans, fontSize: 22, fontWeight: 500, letterSpacing: '0.2em', color: COLOR.faint, opacity: on }}>{text}</span>
  );
  const word = (style: React.CSSProperties, text: string) => <span style={{ display: 'inline-block', ...style }}>{text}</span>;
  return (
    <div style={{ position: 'absolute', inset: 0, display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', gap: 26 }}>
      <div style={{ position: 'relative', height: 30, width: 200, textAlign: 'center' }}>
        <div style={{ position: 'absolute', inset: 0 }}>{label('之前', 1 - fix)}</div>
        <div style={{ position: 'absolute', inset: 0 }}>{label('现在', fix)}</div>
      </div>
      <div style={{ position: 'relative', fontSize: 120, color: COLOR.ink, lineHeight: 1, height: 130 }}>
        {word({ fontFamily: FONT.sans, fontVariationSettings: `'wght' ${800 - 300 * fix}`, fontWeight: 800 - 300 * fix }, '打开')}
        <span style={{ position: 'relative', display: 'inline-block' }}>
          {word({ fontFamily: '"Noto Serif SC"', fontWeight: 300, opacity: 1 - fix, filter: `blur(${fix * 6}px)` }, '仓库')}
          {word(
            { position: 'absolute', left: 0, top: 0, fontFamily: FONT.sans, fontWeight: 500, opacity: fix, filter: `blur(${(1 - fix) * 6}px)` },
            '仓库',
          )}
        </span>
      </div>
      <div style={{ height: 6, width: 360 * fix, borderRadius: 3, background: COLOR.accent, opacity: 0.9 }} />
    </div>
  );
};

/** A shield draws itself, then a check. */
const Notarized: React.FC<{ at: number }> = ({ at }) => {
  const f = useCurrentFrame();
  const shield = interpolate(f, [at + 10, at + 10 + 2 * BEAT], [0, 1], { ...clamp, easing: EASE.inOut });
  const check = interpolate(f, [at + 2.4 * BEAT, at + 3.4 * BEAT], [0, 1], { ...clamp, easing: EASE.out });
  const glow = interpolate(f, [at + 3 * BEAT, at + 4.2 * BEAT], [0, 1], { ...clamp, easing: EASE.out });
  const SHIELD = 'M120 20 L210 52 V128 C210 186 172 228 120 248 C68 228 30 186 30 128 V52 Z';
  return (
    <div style={{ position: 'absolute', inset: 0, display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
      <svg width={260} height={280} viewBox="0 0 240 270" style={{ overflow: 'visible' }}>
        <path d={SHIELD} fill={COLOR.accent} opacity={0.14 * glow} />
        <path
          d={SHIELD}
          fill="none"
          stroke={COLOR.ink}
          strokeWidth={7}
          strokeLinejoin="round"
          pathLength={1}
          strokeDasharray="1 1"
          strokeDashoffset={1 - shield}
        />
        <path
          d="M78 134 L108 164 L166 102"
          fill="none"
          stroke={COLOR.accent}
          strokeWidth={14}
          strokeLinecap="round"
          strokeLinejoin="round"
          pathLength={1}
          strokeDasharray="1 1"
          strokeDashoffset={1 - check}
        />
      </svg>
    </div>
  );
};

/** The two cards after the footage, in stage coordinates. */
export const Cards: React.FC<{ at: number }> = ({ at }) => (
  <>
    <Card at={at} x={100} title="中文显示统一" sub="Windows 上同一个词不再一半粗一半细；界面残留的英文改为中文">
      <Glyphs at={at} />
    </Card>
    <Card at={at + BEAT} x={880} title="macOS 已签名并公证" sub="Developer ID 签名、Apple 公证并钉票，打开时不再提示“无法验证开发者”">
      <Notarized at={at + BEAT} />
    </Card>
  </>
);
