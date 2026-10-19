import { Composition } from 'remotion';
import { FPS, HEIGHT, TOTAL, WIDTH } from './theme';
import { Video } from './Video';

export const Root: React.FC = () => (
  <Composition id="Update" component={Video} durationInFrames={TOTAL} fps={FPS} width={WIDTH} height={HEIGHT} />
);
