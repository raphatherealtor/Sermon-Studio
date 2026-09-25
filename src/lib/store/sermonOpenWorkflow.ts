import type { SermonBackend } from '@/lib/backend/SermonBackend';
import type { SermonDocument } from '@/lib/backend/types';
import { useEditorStore } from './editorStore';
import { saveActiveSermon } from './saveWorkflow';

let switching = false;

/** The single handoff for loading, creating, or duplicating another sermon. */
export async function switchToSermon(
  backend: SermonBackend,
  acquire: () => Promise<SermonDocument>,
  targetId?: string,
): Promise<boolean> {
  if (switching) return false;
  const current = useEditorStore.getState();
  if (targetId && current.activeSermonId === targetId && current.activeDocument) return true;
  if (current.isSaving || current.conflictInfo) return false;

  switching = true;
  try {
    if (current.isDirty && current.activeDocument) {
      if (!(await saveActiveSermon(backend))) return false;
      // An edit made while the save was pending must remain open for another save.
      if (useEditorStore.getState().activeDocument !== current.activeDocument) {
        useEditorStore.getState().markDirty();
        return false;
      }
    }
    const doc = await acquire();
    const store = useEditorStore.getState();
    if (store.activeDocument !== current.activeDocument) {
      if (store.activeDocument) store.markDirty();
      return false;
    }
    store.setDocument(doc);
    store.setActiveSermon(doc.id);
    return true;
  } catch (error) {
    useEditorStore.getState().setSaveError(error instanceof Error ? error.message : 'Unable to open sermon');
    return false;
  } finally {
    switching = false;
  }
}

export function openSermon(backend: SermonBackend, id: string): Promise<boolean> {
  return switchToSermon(backend, () => backend.loadSermon(id), id);
}
