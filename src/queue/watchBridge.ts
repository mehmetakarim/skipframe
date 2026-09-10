import { notifyError } from '../stores/notices';
import { addPaths, queue, startQueue } from '../stores/queue';

/**
 * What happens when the watched folder produces a file.
 *
 * This lives on its own because it is the one place that knows about both the watcher and the
 * queue. Putting it in either would make the two import each other, and a cycle between two
 * stores is the kind of thing that works until the day module evaluation order changes.
 *
 * The listener is installed once and stays installed; it only ever fires while the Rust side is
 * actually watching.
 */

const EVENT = 'skipframe://gcode-appeared';

let installed = false;

export async function installWatchBridge(): Promise<void> {
  if (installed || !('__TAURI_INTERNALS__' in window)) return;
  installed = true;

  const { listen } = await import('@tauri-apps/api/event');
  await listen<string>(EVENT, async (event) => {
    try {
      await addPaths([event.payload]);
      // The setting promises the file is rendered, not merely listed.
      if (!queue.running) await startQueue();
    } catch (e) {
      // Nobody asked for this run, so nobody is watching a screen that could report it. If the
      // watched folder stops working it has to say so wherever the user happens to be.
      notifyError('İzlenen klasördeki dosya işlenemedi', e);
    }
  });
}
