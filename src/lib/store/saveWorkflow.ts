// Shared save workflow (Track I).
//
// The command palette and the editor save control must go through the SAME
// save workflow and state transitions, so the palette can never leave the
// editor showing "Unsaved" after a successful save. EditorPanel already
// performs these exact transitions; this action extracts them so every
// caller shares one path. No second save path is introduced.
//
// Track H owns EditorPanel internals; this module coordinates purely through
// editorStore action/state APIs.

import { useEditorStore } from './editorStore';
import type { SermonBackend } from '@/lib/backend/SermonBackend';

/**
 * Save the active document through the shared workflow.
 *
 * Transitions (identical to the editor's save control):
 *   isSaving=true, saveError=null
 *   → conflict? conflictInfo set (editor shows the resolution banner)
 *   → success? lastSaved set, isDirty=false (clears "Unsaved")
 *   → failure? saveError set
 *   → finally isSaving=false
 *
 * Returns true only when the document was saved without conflict or error.
 */
export async function saveActiveSermon(backend: SermonBackend): Promise<boolean> {
  const {
    activeDocument,
    setSaving,
    setSaveError,
    setLastSaved,
    markClean,
    setConflict,
  } = useEditorStore.getState();
  if (!activeDocument) return false;

  setSaving(true);
  setSaveError(null);
  try {
    const result = await backend.saveSermon(activeDocument);
    if (result.conflict) {
      setConflict({
        localTitle: activeDocument.title,
        localModifiedAt: activeDocument.updatedAt,
        diskModifiedAt: result.conflict.diskModifiedAt,
        diskVersion: result.conflict.diskVersion,
        diskWordCount: result.conflict.diskWordCount,
        sourcePath: activeDocument.sourcePath || '',
        explanation:
          'The file on disk has been modified since your last save. Choose how to reconcile.',
      });
      return false;
    }
    setLastSaved(result.savedAt);
    markClean();
    return true;
  } catch (e: unknown) {
    setSaveError(e instanceof Error ? e.message : 'Save failed');
    return false;
  } finally {
    setSaving(false);
  }
}
