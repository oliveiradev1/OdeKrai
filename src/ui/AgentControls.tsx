import { useEffect, useState } from 'react';
import {
  CalendarDots,
  FileText,
  FolderOpen,
  Globe,
  Lightning,
  Wrench,
} from '@phosphor-icons/react';
import { displayPath } from '../core/display-path';
import {
  formatRemainingApprovalTime,
  getAgentCardState,
  isSafeArtifactPath,
} from '../core/agent-card';
import { RiskBadge } from './RiskBadge';

export type AgentArtifact = { path: string; kind: string };
export type UndoDescriptor = {
  skillId: string;
  stateId: string;
  path?: string | null;
  label: string;
};
export type AgentTimelineEntry = {
  label: string;
  status: string;
  durationMs?: number | null;
  at: number;
};
export type AgentPlanStepData = {
  description: string;
  status: 'pending' | 'in_progress' | 'completed' | 'failed';
};
export type AgentPlanData = {
  title: string;
  steps: AgentPlanStepData[];
  outsidePlanActions: string[];
};

export type AgentActionCardData = {
  id: string;
  skillId: string;
  title: string;
  risk: string;
  decision: 'automática' | 'aprovada' | 'negada' | 'expirada' | 'aguardando';
  status: string;
  summary: string;
  resultSummary?: string;
  diff?: string;
  preview?: string | null;
  artifacts?: AgentArtifact[];
  timeline?: AgentTimelineEntry[];
  durationMs?: number | null;
  approvalExpiresAt?: number | null;
  untrustedResult?: boolean;
  undoDescriptor?: UndoDescriptor | null;
  undoError?: string | null;
  plan?: AgentPlanData | null;
  createdAt?: number;
};

export function AgentControls({
  enabled,
  disabledReason,
  workspacePath,
  running,
  awaitingApproval,
  onToggle,
  onStop,
  onConfigure,
}: {
  enabled: boolean;
  disabledReason: string | null;
  workspacePath: string | null;
  running: boolean;
  awaitingApproval: boolean;
  onToggle: (enabled: boolean) => void;
  onStop: () => void;
  onConfigure: () => void;
}) {
  const unavailable = Boolean(disabledReason);
  return (
    <div className="agent-controls" aria-label="Controles do modo agente">
      <button
        type="button"
        role="switch"
        aria-checked={enabled && !unavailable}
        aria-label="Modo agente"
        aria-describedby={disabledReason ? 'agent-unavailable-reason' : undefined}
        title={disabledReason ?? undefined}
        disabled={unavailable || running}
        onClick={() => onToggle(!enabled)}
      >
        <span className="agent-switch-indicator" aria-hidden="true" />
        <span>Modo agente</span>
        <strong>{unavailable ? 'Indisponível' : enabled ? 'Ligado' : 'Desligado'}</strong>
      </button>
      {disabledReason && (
        <span className="agent-unavailable" id="agent-unavailable-reason" role="status">
          {disabledReason}
        </span>
      )}
      {disabledReason && (
        <button className="agent-configure-button" type="button" onClick={onConfigure}>
          Configurar agente
        </button>
      )}
      {enabled && workspacePath && (
        <span className="agent-workspace" title={workspacePath}>
          Pasta de trabalho: {displayPath(workspacePath)}
        </span>
      )}
      {awaitingApproval && (
        <span className="agent-approval-status" role="status" aria-live="assertive">
          Aguardando aprovação na janela nativa de aprovação.
        </span>
      )}
      {running && (
        <button className="agent-stop-button" type="button" onClick={onStop}>
          Parar agente
        </button>
      )}
    </div>
  );
}

export function AgentActionCard({
  action,
  running = false,
  onStop,
  onReviewApproval,
  onConfigureDiscord,
  onOpenArtifact,
  onRevealArtifact,
  onUndo,
}: {
  action: AgentActionCardData;
  running?: boolean;
  onStop?: () => void;
  onReviewApproval?: () => void;
  onConfigureDiscord?: () => void;
  onOpenArtifact?: (path: string) => void;
  onRevealArtifact?: (path: string) => void;
  onUndo?: (descriptor: UndoDescriptor) => void;
}) {
  const state = getAgentCardState(action);
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    if (state !== 'aguardando aprovação' || !action.approvalExpiresAt) return;
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, [state, action.approvalExpiresAt]);
  const Icon = action.skillId.includes('calendar')
    ? CalendarDots
    : action.skillId.includes('file') ||
        action.skillId.includes('document') ||
        action.skillId.includes('sheet')
      ? FileText
      : action.skillId.includes('workspace')
        ? FolderOpen
        : action.skillId.includes('web') || action.skillId.includes('http')
          ? Globe
          : action.skillId.includes('agent') || action.skillId.includes('computer')
            ? Lightning
            : Wrench;
  const preview = action.preview?.trim();
  const previewExcerpt =
    preview && preview.length > 640 ? `${preview.slice(0, 640).trimEnd()}…` : preview;
  return (
    <article
      role="region"
      className="agent-action-card"
      aria-labelledby={`agent-card-title-${action.id}`}
    >
      <header>
        <Icon aria-hidden="true" size={18} weight="duotone" />
        <h3 id={`agent-card-title-${action.id}`}>{action.title}</h3>
        <RiskBadge risk={action.risk} className="agent-risk-chip" />
      </header>
      <p>{action.summary}</p>
      <p className="agent-card-state" role="status" aria-live="polite">
        Estado: {state}
      </p>
      {state === 'aguardando aprovação' && (
        <div className="agent-card-review">
          <span role="status">
            Aguardando revisão
            {formatRemainingApprovalTime(action.approvalExpiresAt, now)
              ? ` · ${formatRemainingApprovalTime(action.approvalExpiresAt, now)}`
              : ''}
          </span>
          {onReviewApproval && (
            <button type="button" onClick={onReviewApproval}>
              Revisar na janela de aprovação
            </button>
          )}
        </div>
      )}
      {state === 'configuração pendente' && action.skillId === 'discord.send' && (
        <div className="agent-card-configure">
          {onConfigureDiscord && (
            <button className="agent-configure-button" type="button" onClick={onConfigureDiscord}>
              Cadastrar webhook
            </button>
          )}
        </div>
      )}
      {action.resultSummary && (
        <p className="agent-action-result">
          {action.untrustedResult && <strong>Resultado externo · dado não confiável. </strong>}
          {action.resultSummary}
        </p>
      )}
      {action.undoError && (
        <p className="agent-action-undo-error" role="alert">
          Não foi possível desfazer: {action.undoError}
        </p>
      )}
      {action.plan && (
        <section className="agent-plan" aria-label="Plano informativo">
          <p>Este plano é apenas informativo. Cada ação continua sujeita às permissões locais.</p>
          <ol aria-label="Etapas do plano">
            {action.plan.steps.map((step, index) => (
              <li key={`${index}-${step.description}`}>
                <span>{step.description}</span>
                <small>
                  {
                    (
                      {
                        pending: 'Pendente',
                        in_progress: 'Em andamento',
                        completed: 'Concluída',
                        failed: 'Falhou',
                      } as const
                    )[step.status]
                  }
                </small>
              </li>
            ))}
          </ol>
          {action.plan.outsidePlanActions.length > 0 && (
            <div className="agent-plan-outside" role="status" aria-live="polite">
              <strong>Ação fora do plano</strong>
              <ul>
                {action.plan.outsidePlanActions.map((label, index) => (
                  <li key={`${index}-${label}`}>{label}</li>
                ))}
              </ul>
            </div>
          )}
        </section>
      )}
      {action.undoDescriptor && onUndo && (
        <button
          type="button"
          className="agent-undo-button"
          onClick={() => onUndo(action.undoDescriptor!)}
        >
          Desfazer
        </button>
      )}
      <dl>
        <div>
          <dt>Status</dt>
          <dd>{action.status}</dd>
        </div>
        {action.durationMs != null && (
          <div>
            <dt>Duração</dt>
            <dd>{(action.durationMs / 1000).toFixed(1)} s</dd>
          </div>
        )}
      </dl>
      {(action.timeline?.length ?? 0) > 0 && (
        <ol className="agent-card-timeline" aria-label="Etapas da execução">
          {action.timeline?.map((entry, index) => (
            <li key={`${entry.at}-${index}`}>
              <span>{entry.label}</span>
              <small>
                {entry.status}
                {entry.durationMs != null ? ` · ${(entry.durationMs / 1000).toFixed(1)} s` : ''}
              </small>
            </li>
          ))}
        </ol>
      )}
      {running && onStop && (
        <button className="agent-stop-button" type="button" onClick={onStop}>
          Parar
        </button>
      )}
      {preview && (
        <details className="agent-card-preview">
          <summary>Prévia em texto</summary>
          {action.untrustedResult && (
            <p className="agent-untrusted-label">A prévia contém dados não confiáveis.</p>
          )}
          <pre>{previewExcerpt}</pre>
          {preview.length > 640 && (
            <details>
              <summary>Ver completo</summary>
              <pre>{preview}</pre>
            </details>
          )}
        </details>
      )}
      {action.diff && (
        <details>
          <summary>Ver detalhes</summary>
          {action.untrustedResult && (
            <p className="agent-untrusted-label">Conteúdo externo · dado não confiável.</p>
          )}
          <pre>{action.diff}</pre>
        </details>
      )}
      {(action.artifacts ?? [])
        .filter((artifact) => isSafeArtifactPath(artifact.path))
        .map((artifact) => (
          <div className="agent-card-artifact" key={artifact.path}>
            <small>{artifact.kind}</small>
            <span title={artifact.path}>{displayPath(artifact.path)}</span>
            {onOpenArtifact && (
              <button type="button" onClick={() => onOpenArtifact(artifact.path)}>
                Abrir
              </button>
            )}
            {onRevealArtifact && (
              <button type="button" onClick={() => onRevealArtifact(artifact.path)}>
                Mostrar na pasta
              </button>
            )}
          </div>
        ))}
    </article>
  );
}
