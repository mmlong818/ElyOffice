import { AbsoluteFill, interpolate, useCurrentFrame } from 'remotion';
import { Caption } from '../design/Caption';
import { KeyCap } from '../design/KeyCap';
import { Stage } from '../design/Stage';
import { Clip, type Cut, type Shot } from '../footage/Clip';
import { frameOf, Sounds } from '../footage/Sounds';
import { EASE } from '../theme';
import { Cards } from './Cards';

const CUTS: Cut[] = [{ take: 'more', from: 2.6, to: 9.0 }];
const SHOTS: Shot[] = [
  { f: 0, z: 1, x: 1024, y: 576 },
  { f: 40, z: 1.4, x: 731, y: 360 },
  { f: 190, z: 1.4, x: 731, y: 360 },
  { f: 240, z: 1.5, x: 683, y: 780 },
  { f: 296, z: 1.5, x: 683, y: 780 },
  { f: 336, z: 1.5, x: 683, y: 384 },
];
// The cards replace the footage from here.
const CARDS = 372;

export const More: React.FC = () => {
  const f = useCurrentFrame();
  const clear = frameOf(CUTS, 7.38);
  const away = interpolate(f, [CARDS - 20, CARDS + 10], [1, 0], { extrapolateLeft: 'clamp', extrapolateRight: 'clamp', easing: EASE.inOut });
  return (
    <AbsoluteFill>
      <Stage
        enter={{ from: 'ely-show' }}
        exit={{ to: 'dot' }}
        heading={[
          { title: '再开文件，进同一个窗口', sub: '应用已开着时，文件直接开成新标签；侧栏可清除最近文件', app: 'ely-mark' },
          { title: '还有', sub: '中文显示统一 · macOS 安装包已公证', at: CARDS },
        ]}
      >
        {f < CARDS + 10 ? (
          <AbsoluteFill style={{ opacity: away, transform: `scale(${0.97 + 0.03 * away})` }}>
            <Clip cuts={CUTS} shots={SHOTS} />
          </AbsoluteFill>
        ) : null}
        {f >= CARDS - 20 ? <Cards at={CARDS} /> : null}
      </Stage>
      <Caption text="第二次打开：交给已开着的那份，开成新标签" from={frameOf(CUTS, 3.4)} to={frameOf(CUTS, 6.7)} />
      <KeyCap keys={['点击']} from={clear - 20} to={clear + 60} press={[clear]} />
      <Caption text="清除最近文件，重启后仍是空的" from={clear + 10} to={CARDS - 10} />
      <Sounds cuts={CUTS} />
    </AbsoluteFill>
  );
};
