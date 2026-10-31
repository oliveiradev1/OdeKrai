import manifest from './release-notes.generated.json';
import { normalizeReleaseNotesManifest } from './release-notes';

export const releaseNotesManifest = normalizeReleaseNotesManifest(manifest);
