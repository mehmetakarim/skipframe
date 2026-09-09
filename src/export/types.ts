import type { Aspect } from '../stores/scene';

export type ExportFormat = 'mp4' | 'frames';

export interface ExportSettings {
  format: ExportFormat;
  aspect: Aspect;
  fps: number;
  /** 0.5 for a quick look, 2.0 for slow and sharp. Multiplies the aspect's base resolution. */
  scale: number;
  durationS: number;
  openWhenDone: boolean;
}

export type ExportStage =
  'idle' | 'preparing' | 'rendering' | 'finishing' | 'writing' | 'done' | 'cancelled' | 'failed';

/** Turkish labels for the stage line in the progress panel. */
export const STAGE_LABELS: Record<ExportStage, string> = {
  idle: 'Hazır',
  preparing: 'Kodlayıcı hazırlanıyor',
  rendering: 'Kare çizimi',
  finishing: 'Kodlayıcı boşaltılıyor',
  writing: 'Dosya yazılıyor',
  done: 'Bitti',
  cancelled: 'İptal edildi',
  failed: 'Başarısız',
};

export interface ExportProgress {
  stage: ExportStage;
  /** Frames completed. */
  frame: number;
  frameCount: number;
  /** Frames per second of wall-clock time, smoothed. */
  rate: number;
  /** Seconds remaining, or null while there is nothing to estimate from. */
  etaS: number | null;
  bytes: number;
  outputPath: string | null;
  error: string | null;
}

export interface ExportTarget {
  width: number;
  height: number;
}
