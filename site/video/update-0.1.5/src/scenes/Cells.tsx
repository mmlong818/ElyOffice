import { AbsoluteFill } from 'remotion';
import { Caption } from '../design/Caption';
import { KeyCap } from '../design/KeyCap';
import { Stage } from '../design/Stage';
import { Clip, type Cut, type Shot } from '../footage/Clip';
import { frameOf, Sounds } from '../footage/Sounds';

const CUTS: Cut[] = [{ take: 'cells', from: 3.1, to: 13.1 }];
const SHOTS: Shot[] = [
  { f: 0, z: 1, x: 1024, y: 576 },
  { f: 60, z: 1.25, x: 1190, y: 440 },
  { f: 110, z: 1.75, x: 1190, y: 420 },
  { f: 470, z: 1.75, x: 1190, y: 420 },
  { f: 560, z: 1.2, x: 1200, y: 460 },
];

// One badge per key, held until the next one.
const KEYS: { t: number; keys: string[] }[] = [
  { t: 4.26, keys: ['点击'] },
  { t: 5.73, keys: ['Tab'] },
  { t: 6.88, keys: ['↓'] },
  { t: 7.94, keys: ['End'] },
  { t: 8.29, keys: ['⌫'] },
  { t: 10.63, keys: ['Enter'] },
];

export const Cells: React.FC = () => {
  const at = (t: number) => frameOf(CUTS, t);
  const backs = [8.29, 8.5, 8.71].map(at);
  return (
    <AbsoluteFill>
      <Stage
        enter={{ from: 'cell' }}
        exit={{ to: 'ely-doc' }}
        heading={{ title: '表格在格内编辑', sub: '点哪格开哪格，Tab 到下一格，回车写回', app: 'ely-mark' }}
      >
        <Clip cuts={CUTS} shots={SHOTS} />
      </Stage>
      {KEYS.map((k, i) => {
        const from = at(k.t) - 12;
        const to = i + 1 < KEYS.length ? at(KEYS[i + 1].t) - 12 : at(k.t) + 70;
        return <KeyCap key={k.t} keys={k.keys} from={from} to={Math.min(to, from + 110)} press={k.keys[0] === '⌫' ? backs : [at(k.t)]} />;
      })}
      <Caption text="格内显示原文：**粗体** 带着记号" from={at(4.5)} to={at(5.6)} />
      <Caption text="Tab、方向键在格间移动" from={at(5.9)} to={at(7.8)} />
      <Caption text="回车写回，一步撤销" from={at(10.7)} to={at(12.2)} />
      <Sounds cuts={CUTS} />
    </AbsoluteFill>
  );
};
