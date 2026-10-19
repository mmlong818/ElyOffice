import { AbsoluteFill } from 'remotion';
import { Caption } from '../design/Caption';
import { KeyCap } from '../design/KeyCap';
import { Stage } from '../design/Stage';
import { Clip, lay, type Cut, type Shot } from '../footage/Clip';
import { frameOf, Sounds } from '../footage/Sounds';
import { Triangle } from '../footage/Triangle';

const CUTS: Cut[] = [
  { take: 'doc', from: 3.9, to: 9.5 },
  { take: 'doc', from: 11.75, to: 13.85, fade: 12 },
  { take: 'table', from: 3.9, to: 7.4, fade: 14 },
  { take: 'show', from: 3.1, to: 7.57, fade: 14 },
];
const [, drag, table, show] = lay(CUTS);
const SHOTS: Shot[] = [
  { f: 0, z: 1, x: 1024, y: 576 },
  { f: 45, z: 1.2, x: 1100, y: 620 },
  { f: 120, z: 1.45, x: 1260, y: 800 },
  { f: 290, z: 1.45, x: 1260, y: 800 },
  { f: drag.start, z: 1.35, x: 920, y: 640 },
  { f: drag.start + 28, z: 1.65, x: 990, y: 620 },
  { f: table.start - 6, z: 1.65, x: 990, y: 620 },
  { f: table.start + 34, z: 1.45, x: 560, y: 380 },
  { f: show.start - 6, z: 1.45, x: 560, y: 380 },
  { f: show.start + 40, z: 1.5, x: 430, y: 620 },
  { f: show.start + 160, z: 1.5, x: 430, y: 620 },
  { f: show.start + 230, z: 1.15, x: 900, y: 560 },
];

export const RightClick: React.FC = () => {
  const docRight = frameOf(CUTS.slice(0, 1), 4.95);
  const tableRight = table.start + Math.round((4.25 - table.from) * 60);
  const wheel = show.start + Math.round((3.42 - show.from) * 60);
  return (
    <AbsoluteFill>
      <Stage
        enter={{ from: 'ely-doc' }}
        exit={{ to: 'ely-show' }}
        heading={[
          { title: '右键照 Word', sub: '正文、表格、图片各有对应的菜单；拖动边线调列宽行高', app: 'ely-doc' },
          { title: '右键照 Excel', sub: '单元格、行列表头、图形各有对应的菜单', app: 'ely-table', at: table.start },
          { title: '右键照 PowerPoint', sub: '缩略图条可用滚轮滚动，表格边线可以拖动', app: 'ely-show', at: show.start },
        ]}
      >
        <Clip
          cuts={CUTS}
          shots={SHOTS}
          overlay={(cut, t) =>
            cut.take === 'doc' ? (
              <Triangle take="doc" t={t} from={7.29} to={8.19} passed={[742, 784]} note="路过「删除」，不抢走" />
            ) : null
          }
        />
      </Stage>
      <KeyCap keys={['右键']} from={docRight - 20} to={docRight + 75} press={[docRight]} />
      <Caption text="拖动表格边线，调整列宽" from={drag.start + 10} to={table.start - 4} />
      <KeyCap keys={['右键']} from={tableRight - 20} to={tableRight + 75} press={[tableRight]} />
      <Caption text="表头右键：插入、删除、列宽、隐藏" from={tableRight + 30} to={show.start - 4} />
      <KeyCap keys={['滚轮']} from={wheel - 16} to={wheel + 170} press={[wheel, wheel + 96]} />
      <Caption text="缩略图条可以用滚轮滚动" from={wheel + 10} to={wheel + 190} />
      <Sounds cuts={CUTS} />
    </AbsoluteFill>
  );
};
