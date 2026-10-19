import { AbsoluteFill } from 'remotion';
import { Caption } from '../design/Caption';
import { KeyCap } from '../design/KeyCap';
import { Stage } from '../design/Stage';
import { Clip, type Cut, type Shot } from '../footage/Clip';
import { frameOf, Sounds } from '../footage/Sounds';
import { Triangle } from '../footage/Triangle';

// The crossing over 插入 plays at half speed.
const CROSS = { from: 7.09, to: 8.04 };
const CUTS: Cut[] = [
  { take: 'menus', from: 3.6, to: 7.0 },
  { take: 'menus', from: 7.0, to: 8.15, rate: 0.5 },
  { take: 'menus', from: 8.15, to: 12.45 },
];
const SHOTS: Shot[] = [
  { f: 0, z: 1, x: 1024, y: 576 },
  { f: 40, z: 1.15, x: 1120, y: 560 },
  { f: 115, z: 1.45, x: 1380, y: 720 },
  { f: 355, z: 1.45, x: 1380, y: 720 },
  { f: 420, z: 1.25, x: 1150, y: 600 },
  { f: 560, z: 1, x: 1024, y: 576 },
];

export const Menus: React.FC = () => {
  const right = frameOf(CUTS, 4.43);
  const cross = frameOf(CUTS, CROSS.from);
  return (
    <AbsoluteFill>
      <Stage
        enter={{ from: 'ely-mark' }}
        exit={{ to: 'cell' }}
        heading={{ title: '二级菜单更好选', sub: '斜着移向子菜单，路过的项不再抢先弹出', app: 'ely-mark' }}
      >
        <Clip
          cuts={CUTS}
          shots={SHOTS}
          overlay={(_, t) => <Triangle take="menus" t={t} {...CROSS} passed={[654, 696]} note="路过「插入」，不抢走" />}
        />
      </Stage>
      <KeyCap keys={['右键']} from={right - 20} to={right + 75} press={[right]} />
      <Caption text="0.5× · 安全三角：朝子菜单走，三角内的项不切换" from={cross - 30} to={frameOf(CUTS, 8.6)} />
      <Sounds cuts={CUTS} />
    </AbsoluteFill>
  );
};
