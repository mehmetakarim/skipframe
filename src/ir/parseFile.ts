import { invoke } from '@tauri-apps/api/core';

import { decodeIr } from './decode';
import type { Ir } from './types';

export interface ParseResult {
  ir: Ir;
  /** Wall-clock time for the whole round trip, including IPC. */
  elapsedMs: number;
}

/**
 * Ask the Rust side to parse a file and hand back the raw IR buffer.
 *
 * The command returns `tauri::ipc::Response`, which reaches us as an ArrayBuffer — never JSON.
 */
export async function parseFile(
  path: string,
  options: { plate?: number; useCache?: boolean } = {},
): Promise<ParseResult> {
  const started = performance.now();
  const buffer = await invoke<ArrayBuffer>('parse_gcode', {
    path,
    plate: options.plate ?? null,
    useCache: options.useCache ?? true,
  });
  const ir = decodeIr(buffer);
  return { ir, elapsedMs: performance.now() - started };
}

/** Plate numbers inside a `.gcode.3mf`. Plain `.gcode` answers `[1]`. */
export function listPlates(path: string): Promise<number[]> {
  return invoke<number[]>('list_plates', { path });
}

export interface CacheStats {
  entries: number;
  bytes: number;
  path: string;
}

export function cacheStats(): Promise<CacheStats> {
  return invoke<CacheStats>('cache_stats');
}

export function clearCache(): Promise<void> {
  return invoke<void>('clear_cache');
}
