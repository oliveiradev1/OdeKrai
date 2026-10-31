export const RELEASE_NOTES_LAST_SEEN_KEY = 'odekrai:release-notes:last-seen';

export type ReleaseNote = {
  version: string;
  title: string;
  date: string;
  summary: string;
  changes: string[];
};

export type ReleaseNotesManifest = {
  currentVersion: string;
  releases: ReleaseNote[];
};

function stringOrEmpty(value: unknown): string {
  return typeof value === 'string' ? value : '';
}

export function normalizeReleaseNotesManifest(value: unknown): ReleaseNotesManifest {
  if (!value || typeof value !== 'object') return { currentVersion: '', releases: [] };
  const manifest = value as { currentVersion?: unknown; releases?: unknown };
  const releases = Array.isArray(manifest.releases) ? manifest.releases : [];
  return {
    currentVersion: stringOrEmpty(manifest.currentVersion),
    releases: releases.map((value): ReleaseNote => {
      const release = value && typeof value === 'object' ? (value as Record<string, unknown>) : {};
      return {
        version: stringOrEmpty(release.version),
        title: stringOrEmpty(release.title),
        date: stringOrEmpty(release.date),
        summary: stringOrEmpty(release.summary),
        changes: Array.isArray(release.changes) ? release.changes.map(stringOrEmpty) : [],
      };
    }),
  };
}

export type ReleasePromptDecision = {
  show: boolean;
  markSeen: boolean;
  reason: 'new-version' | 'same-version' | 'first-install' | 'disabled' | 'notes-missing';
};

export function decideReleasePrompt({
  currentVersion,
  previousVersion,
  notesAvailable,
  hasExistingPreferences,
  enabled,
}: {
  currentVersion: string;
  previousVersion: string | null;
  notesAvailable: boolean;
  hasExistingPreferences: boolean;
  enabled: boolean;
}): ReleasePromptDecision {
  if (!notesAvailable) return { show: false, markSeen: false, reason: 'notes-missing' };
  if (previousVersion === currentVersion) {
    return { show: false, markSeen: true, reason: 'same-version' };
  }
  if (!previousVersion && !hasExistingPreferences) {
    return { show: false, markSeen: true, reason: 'first-install' };
  }
  if (!enabled) return { show: false, markSeen: true, reason: 'disabled' };
  return { show: true, markSeen: true, reason: 'new-version' };
}
