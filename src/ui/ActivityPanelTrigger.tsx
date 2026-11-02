import { Pulse } from '@phosphor-icons/react';

export function ActivityPanelTrigger({
  count,
  expanded,
  buttonRef,
  onClick,
}: {
  count: number;
  expanded: boolean;
  buttonRef: { current: HTMLButtonElement | null };
  onClick: () => void;
}) {
  const label = count > 0 ? `Atividade; ${count} aprovações pendentes` : 'Abrir atividade';
  return (
    <button
      ref={buttonRef}
      type="button"
      className="activity-panel-trigger"
      aria-label={label}
      aria-expanded={expanded}
      onClick={onClick}
    >
      <Pulse aria-hidden="true" size={16} />
      <span>Atividade</span>
      {count > 0 && <span className="activity-panel-count">{count}</span>}
    </button>
  );
}
