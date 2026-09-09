import { invoke } from '@tauri-apps/api/core';

/**
 * Hand a finished file to the Rust side.
 *
 * The bytes go as the request's raw body, so a 150 MB video crosses the boundary as bytes rather
 * than as a JSON array of numbers. The destination travels percent-encoded in a header, because
 * a command can carry exactly one raw body and headers are ASCII-only.
 */
export async function writeExportFile(path: string, bytes: Uint8Array): Promise<void> {
  await invoke('write_export', bytes, {
    headers: { 'x-sf-path': encodeURIComponent(path) },
  });
}

/** Show the finished file in the OS file manager. */
export async function revealFile(path: string): Promise<void> {
  const { revealItemInDir } = await import('@tauri-apps/plugin-opener');
  await revealItemInDir(path);
}

/** `benchy_0.2mm_PLA.gcode` -> `benchy_0.2mm_PLA`, `plate.gcode.3mf` -> `plate`. */
export function stemOf(fileName: string): string {
  return fileName.replace(/(\.gcode)?\.(gcode|gco|g|3mf)$/i, '');
}
