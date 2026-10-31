import { useEffect, useRef, useState, type KeyboardEvent } from 'react';
import { ArrowSquareOut, ClockCounterClockwise, FolderOpen, Pulse, X } from '@phosphor-icons/react';
import type { AgentActionCardData } from './AgentControls';
import type { HomeEvent } from '../core/home';
import { isSafeArtifactPath } from '../core/agent-card';

export type ActivityAuditSummary = {
  id: string;
  timestamp: number;
  tool: string;
  status: string;
  decision: string;
};

export type UpcomingReminder = {
  title: string;
  whenLocal: string;
  status: 'pending' | 'notification_error' | string;
};

export function activityPanelTabIndex(
  key: string,
  shiftKey: boolean,
  index: number,
  count: number,
) {
  if (key === 'Escape') return -1;
  if (key !== 'Tab' || count === 0) return null;
  if (shiftKey && index === 0) return count - 1;
  if (!shiftKey && index === count - 1) return 0;
  return null;
}

export function toggleActivityPanel(open: boolean) {
  return !open;
}

export function shouldLoadPanelAudit(open: boolean) {
  return open;
}

export function focusActivityPanel(
  trigger: { focus: () => void },
  initialFocus: { focus: () => void },
) {
  initialFocus.focus();
  return () => trigger.focus();
}

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

function formatWhen(value: string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return 'Horário indisponível';
  return new Intl.DateTimeFormat('pt-BR', { dateStyle: 'short', timeStyle: 'short' }).format(date);
}

function auditToolLabel(tool: string) {
  const labels: Record<string, string> = {
    'files.write': 'Salvar arquivo',
    'files.append': 'Acrescentar arquivo',
    'files.rename': 'Renomear arquivo',
    'files.restore': 'Restaurar arquivo',
    'calendar.create_event': 'Criar evento',
    'calendar.list_events': 'Consultar agenda',
    'reminder.create': 'Criar lembrete',
    'reminder.cancel': 'Cancelar lembrete',
    'workspace.read_file': 'Ler arquivo',
    'workspace.write_file': 'Salvar arquivo',
    'plan.propose': 'Propor plano',
  };
  return labels[tool] ?? 'Ação do OdeKrai';
}

function auditStateLabel(status: string, decision: string) {
  if (status === 'completed') return 'Concluída';
  if (status === 'failed') return 'Falhou';
  if (status === 'denied') return 'Bloqueada';
  if (decision === 'ask_user') return 'Aguardando você';
  return 'Interrompida';
}

export function ActivitySidePanel({
  open = true,
  pendingApprovalCount,
  running,
  awaitingApproval,
  currentAction,
  events,
  reminderCount,
  reminders,
  remindersLoading,
  upcomingReminderUnavailable,
  scheduleUnavailable,
  reminderUnavailable,
  actions,
  artifactHistoryAvailable,
  audit,
  scheduleLoading,
  auditLoading,
  auditUnavailable,
  triggerRef,
  onClose,
  onStop,
  onReview,
  onOpenArtifact,
  onRevealArtifact,
  onViewAll,
}: {
  open?: boolean;
  pendingApprovalCount: number;
  running: boolean;
  awaitingApproval: boolean;
  currentAction: AgentActionCardData | null;
  events: HomeEvent[];
  reminderCount: number;
  reminders: UpcomingReminder[];
  remindersLoading: boolean;
  upcomingReminderUnavailable: boolean;
  scheduleUnavailable: boolean;
  reminderUnavailable: boolean;
  actions: AgentActionCardData[];
  artifactHistoryAvailable: boolean;
  audit: ActivityAuditSummary[];
  scheduleLoading: boolean;
  auditLoading: boolean;
  auditUnavailable: boolean;
  triggerRef: { current: HTMLButtonElement | null };
  onClose: () => void;
  onStop: () => void;
  onReview: () => void;
  onOpenArtifact: (path: string) => void;
  onRevealArtifact: (path: string) => void;
  onViewAll: () => void;
}) {
  const panelRef = useRef<HTMLElement>(null);
  const closeRef = useRef<HTMLButtonElement>(null);
  const [narrow, setNarrow] = useState(
    () => typeof window !== 'undefined' && window.innerWidth < 1100,
  );

  useEffect(() => {
    const resize = () => setNarrow(window.innerWidth < 1100);
    window.addEventListener('resize', resize);
    return () => window.removeEventListener('resize', resize);
  }, []);

  useEffect(() => {
    if (!open) return;
    const restoreFocus = focusActivityPanel(
      { focus: () => triggerRef.current?.focus() },
      { focus: () => closeRef.current?.focus() },
    );
    return restoreFocus;
  }, [open, triggerRef]);

  function handleKeyDown(event: KeyboardEvent<HTMLElement>) {
    if (event.key === 'Escape') {
      event.preventDefault();
      onClose();
      return;
    }
    if (!narrow || event.key !== 'Tab') return;
    const focusable = Array.from(
      panelRef.current?.querySelectorAll<HTMLElement>(
        'button:not(:disabled), a[href], [tabindex]:not([tabindex="-1"])',
      ) ?? [],
    );
    const index = focusable.indexOf(document.activeElement as HTMLElement);
    const destination = activityPanelTabIndex(event.key, event.shiftKey, index, focusable.length);
    if (destination !== null && destination >= 0) {
      event.preventDefault();
      focusable[destination]?.focus();
    }
  }

  const artifacts = actions
    .flatMap((action) =>
      (action.artifacts ?? []).map((artifact) => ({ ...artifact, actionId: action.id })),
    )
    .filter((artifact) => isSafeArtifactPath(artifact.path))
    .slice(-5)
    .reverse();

  if (!open) return null;

  return (
    <div className={`activity-panel-layer${narrow ? ' activity-panel-layer--modal' : ''}`}>
      {narrow && (
        <button
          type="button"
          className="activity-panel-scrim"
          aria-label="Fechar atividade"
          onClick={onClose}
        />
      )}
      <aside
        id="activity-side-panel"
        ref={panelRef}
        className="activity-side-panel"
        role={narrow ? 'dialog' : 'complementary'}
        aria-modal={narrow ? true : undefined}
        aria-labelledby="activity-side-panel-title"
        onKeyDown={handleKeyDown}
      >
        <header className="activity-side-panel-header">
          <div>
            <h2 id="activity-side-panel-title">Atividade</h2>
            <span className="activity-local-label">Execução: Local</span>
          </div>
          <button
            ref={closeRef}
            type="button"
            aria-label="Fechar painel de atividade"
            onClick={onClose}
          >
            <X aria-hidden="true" size={18} />
          </button>
        </header>

        <div className="activity-side-panel-content">
          <section aria-labelledby="activity-running-title">
            <h3 id="activity-running-title">
              <Pulse aria-hidden="true" size={16} /> Em execução
            </h3>
            {running ? (
              <div className="activity-panel-card">
                <strong>{currentAction?.title ?? 'O agente está trabalhando'}</strong>
                {awaitingApproval && <p>Execução pausada enquanto aguarda aprovação.</p>}
                {currentAction?.timeline?.length ? (
                  <ol>
                    {currentAction.timeline.slice(-3).map((step, index) => (
                      <li key={`${step.at}-${index}`}>
                        {step.label} <small>{step.status}</small>
                      </li>
                    ))}
                  </ol>
                ) : (
                  <p>Preparando a próxima etapa.</p>
                )}
                <button type="button" className="activity-panel-action" onClick={onStop}>
                  Parar
                </button>
              </div>
            ) : (
              <p className="activity-panel-empty">Nenhuma execução em andamento.</p>
            )}
          </section>

          <section aria-labelledby="activity-awaiting-title">
            <h3 id="activity-awaiting-title">Aguardando você</h3>
            {pendingApprovalCount > 0 ? (
              <div className="activity-panel-card">
                <p>
                  {pendingApprovalCount}{' '}
                  {pendingApprovalCount === 1 ? 'aprovação pendente' : 'aprovações pendentes'}.
                </p>
                <button type="button" className="activity-panel-action" onClick={onReview}>
                  Revisar
                </button>
              </div>
            ) : (
              <p className="activity-panel-empty">Nenhuma aprovação pendente.</p>
            )}
          </section>

          <section aria-labelledby="activity-scheduled-title">
            <h3 id="activity-scheduled-title">
              Agendado <small>Próximas 24 horas</small>
            </h3>
            {scheduleLoading ? (
              <p className="activity-panel-empty">Carregando agenda…</p>
            ) : scheduleUnavailable ? (
              <p className="activity-panel-empty">
                A agenda não está disponível neste momento ou a habilidade de leitura está
                desativada.
              </p>
            ) : (
              <>
                {events.length ? (
                  <ul className="activity-panel-list">
                    {events.slice(0, 5).map((event) => (
                      <li key={event.id}>
                        <strong>{event.title}</strong>
                        <time>{formatWhen(event.start)}</time>
                      </li>
                    ))}
                  </ul>
                ) : (
                  <p className="activity-panel-empty">Nenhum evento nas próximas 24 horas.</p>
                )}
                {reminderUnavailable ? (
                  <p className="activity-panel-empty">
                    O resumo dos lembretes não está disponível ou a habilidade está desativada.
                  </p>
                ) : upcomingReminderUnavailable ? (
                  <p className="activity-panel-empty">
                    Não foi possível carregar os próximos lembretes.
                  </p>
                ) : remindersLoading ? (
                  <p className="activity-panel-empty">Carregando lembretes…</p>
                ) : reminders.length ? (
                  <>
                    <p className="activity-panel-reminders">
                      {reminderCount}{' '}
                      {reminderCount === 1 ? 'lembrete pendente' : 'lembretes pendentes'}
                    </p>
                    <ul className="activity-panel-list">
                      {reminders.map((reminder, index) => (
                        <li key={`${reminder.whenLocal}-${index}`}>
                          <strong>{reminder.title}</strong>
                          <time dateTime={reminder.whenLocal}>
                            {formatWhen(reminder.whenLocal)}
                          </time>
                          {reminder.status === 'notification_error' && (
                            <span>Notificação não exibida</span>
                          )}
                        </li>
                      ))}
                    </ul>
                  </>
                ) : (
                  <>
                    {reminderCount > 0 && (
                      <p className="activity-panel-reminders">
                        {reminderCount}{' '}
                        {reminderCount === 1 ? 'lembrete pendente' : 'lembretes pendentes'}
                      </p>
                    )}
                    <p className="activity-panel-empty">Nenhum lembrete nas próximas 24 horas.</p>
                  </>
                )}
              </>
            )}
          </section>

          <section aria-labelledby="activity-files-title">
            <h3 id="activity-files-title">
              <FolderOpen aria-hidden="true" size={16} /> Arquivos recentes
            </h3>
            {artifacts.length ? (
              <ul className="activity-panel-list">
                {artifacts.map((artifact) => (
                  <li key={`${artifact.actionId}-${artifact.path}`}>
                    <strong title={artifact.path}>{artifact.path.split(/[\\/]/).at(-1)}</strong>
                    <div>
                      <button type="button" onClick={() => onOpenArtifact(artifact.path)}>
                        Abrir
                      </button>
                      <button type="button" onClick={() => onRevealArtifact(artifact.path)}>
                        Mostrar na pasta
                      </button>
                    </div>
                  </li>
                ))}
              </ul>
            ) : (
              <p className="activity-panel-empty">
                {artifactHistoryAvailable
                  ? 'Nenhum arquivo recente nesta conversa.'
                  : 'Abra uma conversa para consultar arquivos recentes.'}
              </p>
            )}
          </section>

          <section aria-labelledby="activity-history-title">
            <h3 id="activity-history-title">
              <ClockCounterClockwise aria-hidden="true" size={16} /> Histórico resumido
            </h3>
            {auditUnavailable ? (
              <p className="activity-panel-empty">
                Não foi possível carregar o histórico de atividade.
              </p>
            ) : audit.length ? (
              <ul className="activity-panel-list">
                {audit.slice(0, 5).map((entry) => (
                  <li key={entry.id}>
                    <strong>{auditToolLabel(entry.tool)}</strong>
                    <span>{auditStateLabel(entry.status, entry.decision)}</span>
                    <time>{formatWhen(new Date(entry.timestamp * 1000).toISOString())}</time>
                  </li>
                ))}
              </ul>
            ) : (
              <p className="activity-panel-empty">
                {auditLoading ? 'Carregando histórico…' : 'Nenhuma ação recente registrada.'}
              </p>
            )}
          </section>

          <button type="button" className="activity-panel-view-all" onClick={onViewAll}>
            Ver tudo <ArrowSquareOut aria-hidden="true" size={15} />
          </button>
        </div>
      </aside>
    </div>
  );
}
