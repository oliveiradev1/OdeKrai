import { releaseNotesManifest } from '../core/release-notes-manifest';
import { normalizeReleaseNotesManifest } from '../core/release-notes';
import { Button } from './DesignSystem';
import { Modal } from './Modal';

export function ReleaseNotesContent({
  manifest = releaseNotesManifest,
  showAllVersions,
  onShowAllVersions,
  onShowCurrent,
  onClose,
}: {
  manifest?: unknown;
  showAllVersions: boolean;
  onShowAllVersions: () => void;
  onShowCurrent: () => void;
  onClose: () => void;
}) {
  const safeManifest = normalizeReleaseNotesManifest(manifest);
  const current = safeManifest.releases.find(
    (release) => safeManifest.currentVersion && release.version === safeManifest.currentVersion,
  );

  const releases = showAllVersions ? safeManifest.releases : current ? [current] : [];
  if (releases.length === 0) {
    return <p role="status">As notas de versão não estão disponíveis.</p>;
  }
  return (
    <div
      className="release-notes-content"
      aria-label={
        showAllVersions
          ? 'Notas de todas as versões'
          : `Novidades da versão ${current?.version || safeManifest.currentVersion || '—'}`
      }
    >
      {releases.map((release) => (
        <article className="release-notes-entry" key={release.version}>
          {showAllVersions && <h3>Versão {release.version || '—'}</h3>}
          {release.date && <p className="release-notes-date">{release.date}</p>}
          {release.summary && <p>{release.summary}</p>}
          {release.changes.some((change) => change.trim()) && (
            <ul>
              {release.changes
                .filter((change) => change.trim())
                .map((change, index) => (
                  <li key={`${release.version}-${index}`}>{change}</li>
                ))}
            </ul>
          )}
        </article>
      ))}
      <footer className="release-notes-actions">
        {showAllVersions ? (
          <Button type="button" variant="secondary" onClick={onShowCurrent}>
            Voltar às novidades
          </Button>
        ) : (
          <Button type="button" variant="secondary" onClick={onShowAllVersions}>
            Ver todas as versões
          </Button>
        )}
        <Button type="button" variant="primary" onClick={onClose}>
          Fechar
        </Button>
      </footer>
    </div>
  );
}

export function ReleaseNotesModal({
  open,
  showAllVersions,
  onShowAllVersions,
  onShowCurrent,
  onClose,
}: {
  open: boolean;
  showAllVersions: boolean;
  onShowAllVersions: () => void;
  onShowCurrent: () => void;
  onClose: () => void;
}) {
  const safeManifest = normalizeReleaseNotesManifest(releaseNotesManifest);
  const current = safeManifest.releases.find(
    (release) => safeManifest.currentVersion && release.version === safeManifest.currentVersion,
  );

  return (
    <Modal
      open={open}
      title={
        showAllVersions || !current
          ? 'Notas de versão do OdeKrai'
          : `O que há de novo na versão ${current.version}`
      }
      onClose={onClose}
    >
      <ReleaseNotesContent
        showAllVersions={showAllVersions}
        onShowAllVersions={onShowAllVersions}
        onShowCurrent={onShowCurrent}
        onClose={onClose}
      />
    </Modal>
  );
}
