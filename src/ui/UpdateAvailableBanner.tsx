export type UpdateCheckResult = {
  current_version: string;
  available_version: string | null;
  release_url: string | null;
  notes_summary: string | null;
  check_succeeded: boolean;
};

export function UpdateAvailableBanner({
  update,
  onOpen,
  onDismiss,
}: {
  update: UpdateCheckResult;
  onOpen: () => void;
  onDismiss: () => void;
}) {
  if (!update.check_succeeded || !update.available_version) return null;
  return (
    <aside className="update-available-banner" role="status" aria-live="polite">
      <div>
        <strong>Versão {update.available_version} disponível</strong>
        {update.notes_summary && <p>{update.notes_summary}</p>}
      </div>
      <button type="button" className="secondary-button small-button" onClick={onOpen}>
        Ver e baixar
      </button>
      <button
        type="button"
        className="update-available-dismiss"
        aria-label="Dispensar aviso de atualização"
        onClick={onDismiss}
      >
        ×
      </button>
    </aside>
  );
}
