import { queue, type QueueJob } from '../stores/queue';

/**
 * Fill the queue with rows covering every state, for working on the screen in a browser tab.
 * Development only — the bundler drops it from a build.
 */
export function seedPreviewJobs(): void {
  if (!import.meta.env.DEV) return;
  const base = '/Users/mk/Movies/SkipFrame';
  const jobs: QueueJob[] = [
    {
      id: 'preview-1',
      path: '/tmp/vazo_spiral_0.3mm.gcode',
      name: 'vazo_spiral_0.3mm.gcode',
      layers: 842,
      bytes: 21_400_000,
      dialect: 'PrusaSlicer',
      warnings: [],
      status: 'rendering',
      frame: 243,
      frameCount: 380,
      etaS: 54,
      outputPath: `${base}/vazo_spiral.mp4`,
      error: null,
      tookS: null,
    },
    {
      id: 'preview-2',
      path: '/tmp/dis_carki_pa6.gcode',
      name: 'dis_carki_pa6.gcode',
      layers: 318,
      bytes: 9_100_000,
      dialect: 'OrcaSlicer',
      warnings: [],
      status: 'pending',
      frame: 0,
      frameCount: 380,
      etaS: null,
      outputPath: `${base}/dis_carki_pa6.mp4`,
      error: null,
      tookS: null,
    },
    {
      id: 'preview-3',
      path: '/tmp/kulaklik_askisi.gcode',
      name: 'kulaklik_askisi.gcode',
      layers: 196,
      bytes: 4_700_000,
      dialect: 'Cura',
      warnings: [],
      status: 'done',
      frame: 380,
      frameCount: 380,
      etaS: null,
      outputPath: `${base}/kulaklik_askisi.mp4`,
      error: null,
      tookS: 47,
    },
    {
      id: 'preview-4',
      path: '/tmp/kapak_v3.gcode',
      name: 'kapak_v3.gcode',
      layers: 318,
      bytes: 6_200_000,
      dialect: 'Unknown',
      warnings: ['Slicer could not be identified.'],
      status: 'pending',
      frame: 0,
      frameCount: 380,
      etaS: null,
      outputPath: `${base}/kapak_v3.mp4`,
      error: null,
      tookS: null,
    },
  ];
  queue.jobs = jobs;
  queue.running = true;
}
