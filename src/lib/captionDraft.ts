import type { Ir } from '../ir/types';
import { layerHeightOf } from '../render/beadGeometry';
import { stemOf } from '../export/writeFile';
import { decimal, integer } from './format';

/**
 * A first draft of a post's title and description, written from the print itself.
 *
 * Only what the parser actually knows goes in: layer count and height, the slicer's time and
 * filament estimates, the printer, the number of extruders. The design's sample text mentions a
 * nozzle temperature and a print speed; the IR has neither, and a public post is the last place
 * to invent a number. Anything else is the user's to write.
 *
 * No line about SkipFrame is added. It is the user's post.
 */

export interface CaptionDraft {
  title: string;
  description: string;
}

export function draftCaption(ir: Ir): CaptionDraft {
  const meta = ir.meta;
  const name = readableName(meta.sourceName);
  const height = layerHeightOf(ir);

  const title = `${name} · ${integer(ir.layerCount)} katman`;

  const lines = [
    `${decimal(height, 2)} mm katman yüksekliği, ${integer(ir.layerCount)} katman.`,
    meta.estimatedTimeS ? `Tahmini baskı süresi: ${spokenDuration(meta.estimatedTimeS)}.` : null,
    meta.filamentGrams ? `Filament: ${decimal(meta.filamentGrams, 1)} g.` : null,
    meta.printerModel ? `Yazıcı: ${meta.printerModel}.` : null,
    meta.toolCount > 1 ? `${meta.toolCount} renkli baskı.` : null,
  ];

  return { title, description: lines.filter(Boolean).join('\n') };
}

/** `erglagalvabamboo_ABS_3h45m.gcode` -> `erglagalvabamboo ABS 3h45m`. */
function readableName(sourceName: string): string {
  const stem = stemOf(sourceName || 'SkipFrame')
    .replace(/[_]+/g, ' ')
    .trim();
  return stem || 'SkipFrame';
}

/**
 * `3 saat 45 dakika`. The app's own shorthand (`3s 45dk`) suits a rail of figures; a sentence
 * someone reads under a video wants the words.
 */
function spokenDuration(seconds: number): string {
  const total = Math.max(0, Math.round(seconds / 60));
  const h = Math.floor(total / 60);
  const m = total % 60;
  if (h === 0) return `${m} dakika`;
  if (m === 0) return `${h} saat`;
  return `${h} saat ${m} dakika`;
}
