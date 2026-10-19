import { AbsoluteFill, Audio, Sequence, getStaticFiles, staticFile } from 'remotion';
import './design/fonts';
import { Background } from './design/Background';
import { Intro } from './scenes/Intro';
import { Outro } from './scenes/Outro';
import { Menus } from './scenes/Menus';
import { Cells } from './scenes/Cells';
import { RightClick } from './scenes/RightClick';
import { More } from './scenes/More';
import { SECTION } from './theme';

// Set false to render without the score.
const MUSIC = true;
const MUSIC_FILE = 'audio/music.wav';
const hasMusic = () => getStaticFiles().some((file) => file.name.split('\\').join('/') === MUSIC_FILE);

// Whooshes at each morph, impact on the outro downbeat.
const MORPHS = [300, 840, 1440, 2340, 3090];

const at = (section: { from: number; duration: number }) => ({ from: section.from, durationInFrames: section.duration });

export const Video: React.FC = () => (
  <AbsoluteFill>
    <Background />
    <Sequence {...at(SECTION.intro)} name="Intro">
      <Intro />
    </Sequence>
    <Sequence {...at(SECTION.menus)} name="Menus">
      <Menus />
    </Sequence>
    <Sequence {...at(SECTION.cells)} name="Cells">
      <Cells />
    </Sequence>
    <Sequence {...at(SECTION.rightClick)} name="RightClick">
      <RightClick />
    </Sequence>
    <Sequence {...at(SECTION.more)} name="More">
      <More />
    </Sequence>
    <Sequence {...at(SECTION.outro)} name="Outro">
      <Outro />
    </Sequence>
    {MORPHS.map((frame) => (
      <Sequence key={frame} from={frame} durationInFrames={45} name="whoosh">
        <Audio src={staticFile('audio/sfx/whoosh.wav')} volume={0.35} />
      </Sequence>
    ))}
    <Sequence from={3150} durationInFrames={90} name="impact">
      <Audio src={staticFile('audio/sfx/impact.wav')} volume={0.4} />
    </Sequence>
    {MUSIC && hasMusic() ? <Audio src={staticFile(MUSIC_FILE)} /> : null}
  </AbsoluteFill>
);
