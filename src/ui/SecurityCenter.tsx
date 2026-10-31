import { useEffect, useMemo, useRef, useState } from 'react';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { loadSettings } from '../core/settings';
import { normalizeSoundPreferences, playSound, SoundTriggerGate } from '../core/sounds';
import { Button, PageHeader, SelectableCard } from './DesignSystem';
import { RiskBadge, type RiskLevel } from './RiskBadge';
import { HistoryDeletionSettings } from './HistoryDeletionSettings';
import { PrivacyConfigurations } from './PrivacyConfigurations';
import type { AppSettings } from '../core/settings';

type Mode = 'sempre_perguntar' | 'aprovar_por_mim' | 'controle_total';
type ToolOverride = { tool: string; morePermissive: boolean };
type PermissionSettings = {
  mode: Mode;
  shortcut: string;
  shortcutError: string | null;
  halted: boolean;
  approvalTtlSeconds: number;
  overrides: ToolOverride[];
};
export type AuditEntry = {
  timestamp: number;
  id: string;
  tool: string;
  risk: RiskLevel;
  mode: Mode;
  origin: string;
  decision: string;
  ruleId: string;
  approval: string;
  latencyMs: number;
  paramsSummary: string;
  status: string;
  error: string | null;
  previousHash: string;
  hash: string;
};

export function auditRuleLabel(ruleId: string): string {
  return ruleId.trim() || 'configuração ausente';
}
type Pending = {
  id: string;
  tool: string;
  risk: RiskLevel;
  origin: string;
  summary: string;
  preview: string;
  previewImage: string | null;
  fullPreview: boolean;
  expiresAt: number;
  displayHash: string;
};

const modes: { id: Mode; title: string; description: string }[] = [
  {
    id: 'aprovar_por_mim',
    title: 'Aprovar por mim',
    description:
      'Leituras e alterações reversíveis em pastas permitidas seguem automaticamente. Mensagens e ações destrutivas pedem aprovação.',
  },
  {
    id: 'sempre_perguntar',
    title: 'Sempre perguntar',
    description:
      'Toda alteração ou envio de mensagem pede aprovação. Leituras em pastas permitidas são permitidas.',
  },
  {
    id: 'controle_total',
    title: 'Controle total',
    description:
      'Executa ações permitidas sem perguntar. Pagamentos, segredos e caminhos bloqueados continuam bloqueados ou exigem aprovação.',
  },
];
const risks: { id: RiskLevel; title: string }[] = [
  { id: 'leitura', title: 'Leitura de arquivos' },
  { id: 'escrita_reversivel', title: 'Escrita reversível' },
  { id: 'comunicacao_externa', title: 'Comunicação externa' },
  { id: 'destrutivo_financeiro', title: 'Destrutivo ou financeiro' },
  { id: 'execucao_de_codigo', title: 'Execução de código (alto)' },
  { id: 'captura_sensivel', title: 'Captura sensível' },
];
const tools = [
  ['workspace.read_file', 'Ler arquivo em pasta permitida'],
  ['workspace.write_file', 'Salvar arquivo em pasta permitida'],
  ['workspace.delete_file', 'Mover arquivo permitido para a lixeira'],
  ['simulated.send_message', 'Simular envio de mensagem'],
  ['simulated.payment', 'Pagamento (bloqueado)'],
] as const;

function toolLabel(tool: string): string {
  return tools.find(([id]) => id === tool)?.[1] ?? 'Ação do OdeKrai';
}

function decision(mode: Mode, risk: RiskLevel): string {
  if (risk === 'leitura') return 'Permitir em pastas permitidas';
  if (risk === 'execucao_de_codigo') return 'Sempre pedir aprovação';
  if (risk === 'captura_sensivel') return 'Sempre pedir aprovação';
  if (mode === 'sempre_perguntar') return 'Pedir aprovação';
  if (mode === 'aprovar_por_mim')
    return risk === 'escrita_reversivel' ? 'Permitir em pastas permitidas' : 'Pedir aprovação';
  if (risk === 'destrutivo_financeiro')
    return 'Na lixeira: permitir em pastas permitidas; caminhos bloqueados continuam indisponíveis';
  return 'Permitir (origem externa sempre pede aprovação)';
}

export function SecurityCenter({
  page,
  embedded = false,
  onOpenFullPage,
  onDeleteAllHistory,
  appSettings,
  updateAppSettings,
  sharedAudit,
}: {
  page: 'permissions' | 'activity';
  embedded?: boolean;
  onOpenFullPage?: () => void;
  onDeleteAllHistory?: () => Promise<boolean>;
  appSettings?: AppSettings;
  updateAppSettings?: (settings: AppSettings) => void;
  sharedAudit?: {
    entries: AuditEntry[];
    verified: boolean | null;
    loading: boolean;
    unavailable?: boolean;
  };
}) {
  const [settings, setSettings] = useState<PermissionSettings | null>(null);
  const [entries, setEntries] = useState<AuditEntry[]>([]);
  const [verified, setVerified] = useState<boolean | null>(null);
  const [error, setError] = useState('');
  const [shortcut, setShortcut] = useState('');
  const [filter, setFilter] = useState('');

  async function refresh() {
    try {
      const current = await invoke<PermissionSettings>('get_permission_settings');
      setSettings(current);
      setShortcut(current.shortcut);
      if (page === 'activity' && !sharedAudit) {
        const [audit, valid] = await Promise.all([
          invoke<AuditEntry[]>('list_audit'),
          invoke<boolean>('verify_audit'),
        ]);
        setEntries(audit);
        setVerified(valid);
      }
      setError('');
    } catch (cause) {
      setError(
        typeof cause === 'string'
          ? cause
          : 'Não foi possível ler as configurações de segurança. Configure a pasta do agente para usar o log de atividade.',
      );
    }
  }

  useEffect(() => {
    if (isTauri()) void refresh();
  }, [page]);

  async function changeMode(mode: Mode) {
    try {
      await invoke('set_permission_mode', { mode });
      await refresh();
    } catch (cause) {
      setError(typeof cause === 'string' ? cause : 'O modo não foi alterado.');
    }
  }

  async function changeOverride(tool: string, morePermissive: boolean) {
    try {
      await invoke('set_tool_override', { tool, morePermissive });
      await refresh();
    } catch (cause) {
      setError(typeof cause === 'string' ? cause : 'A exceção não foi alterada.');
    }
  }

  async function changeShortcut() {
    try {
      await invoke('set_kill_shortcut', { shortcut });
      await refresh();
    } catch (cause) {
      setError(typeof cause === 'string' ? cause : 'O atalho não foi registrado.');
    }
  }

  async function changeApprovalTtl(seconds: number) {
    try {
      await invoke('set_approval_ttl', { seconds });
      await refresh();
    } catch (cause) {
      setError(typeof cause === 'string' ? cause : 'O prazo de aprovação não foi alterado.');
    }
  }

  async function stopOrResume() {
    try {
      await invoke(settings?.halted ? 'resume_all' : 'halt_all');
      await refresh();
    } catch (cause) {
      setError(typeof cause === 'string' ? cause : 'Não foi possível alterar o estado do agente.');
    }
  }

  async function runSample(action: object) {
    try {
      const result = await invoke<{ message: string; status: string }>('submit_simulated_action', {
        request: { action },
      });
      setError(result.message);
      if (result.status === 'completed') {
        void playSound('action_done', normalizeSoundPreferences(loadSettings().companion.sounds));
      }
      if (page === 'activity') await refresh();
    } catch (cause) {
      setError(typeof cause === 'string' ? cause : 'A simulação não foi concluída.');
    }
  }

  const displayedEntries = sharedAudit?.entries ?? entries;
  const displayedVerified = sharedAudit?.verified ?? verified;
  const visibleEntries = useMemo(
    () =>
      displayedEntries.filter((entry) =>
        `${entry.tool} ${entry.status} ${entry.origin} ${entry.decision}`
          .toLowerCase()
          .includes(filter.toLowerCase()),
      ),
    [displayedEntries, filter],
  );

  return (
    <div
      id={page === 'activity' ? 'activity-log' : undefined}
      className={
        embedded ? 'settings-embedded-content security-page' : 'page subpage security-page'
      }
    >
      {!embedded && (
        <PageHeader
          eyebrow="SEGURANÇA LOCAL"
          title={page === 'permissions' ? 'Permissões' : 'Atividade'}
          subtitle="Acompanhe as confirmações e os registros de segurança do OdeKrai."
          actions={
            settings && (
              <span className={`state-chip ${settings.halted ? 'warning' : 'connected'}`}>
                {settings.halted ? 'PARADO' : `MODO ${settings.mode.toUpperCase()}`}
              </span>
            )
          }
        />
      )}
      {error && (
        <div className="security-notice" role="status">
          {error}
        </div>
      )}
      {!isTauri() && (
        <div className="security-notice">
          Permissões e auditoria estão disponíveis no aplicativo nativo.
        </div>
      )}
      {page === 'permissions' && settings && (
        <>
          <section id="permissions-mode" className="settings-panel">
            <div className="panel-heading">
              <div>
                <h3>Modo global</h3>
                <p>Defina quando o OdeKrai precisa pedir sua confirmação.</p>
              </div>
            </div>
            <div className="permission-mode-grid">
              {modes.map((mode) => (
                <SelectableCard
                  key={mode.id}
                  className={`permission-mode ${settings.mode === mode.id ? 'selected' : ''}`}
                  selected={settings.mode === mode.id}
                  onClick={() => void changeMode(mode.id)}
                >
                  <strong>{mode.title}</strong>
                  <span>{mode.description}</span>
                </SelectableCard>
              ))}
            </div>
            <div id="permissions-matrix" className="permission-matrix">
              <table>
                <thead>
                  <tr>
                    <th>Modo</th>
                    {risks.map((risk) => (
                      <th key={risk.id}>{risk.title}</th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {modes.map((mode) => (
                    <tr key={mode.id}>
                      <th>{mode.title}</th>
                      {risks.map((risk) => (
                        <td key={risk.id}>{decision(mode.id, risk.id)}</td>
                      ))}
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </section>
          {onDeleteAllHistory && (
            <section id="permissions-data" className="settings-panel">
              <h3>Apagar conversas</h3>
              <p>Você pode apagar o histórico deste dispositivo quando quiser.</p>
              <HistoryDeletionSettings onConfirm={onDeleteAllHistory} />
            </section>
          )}
          {appSettings && updateAppSettings && (
            <PrivacyConfigurations settings={appSettings} update={updateAppSettings} />
          )}
          <section className="settings-panel">
            <div className="panel-heading">
              <div>
                <h3>Exceções por ação</h3>
                <p>
                  Uma exceção permissiva pede confirmação nativa do Windows. Bloqueios fixos e
                  conteúdo externo não podem ser ignorados.
                </p>
              </div>
            </div>
            {tools.map(([tool, label]) => {
              const fixed = tool === 'workspace.read_file' || tool === 'simulated.payment';
              const enabled = settings.overrides.some(
                (item) => item.tool === tool && item.morePermissive,
              );
              return (
                <div className="preference-row" key={tool}>
                  <div>
                    <strong>{label}</strong>
                    <span>
                      {fixed
                        ? tool === 'workspace.read_file'
                          ? 'Leitura local já é permitida pela política.'
                          : 'Bloqueio fixo: nenhuma exceção é aceita.'
                        : label}
                    </span>
                  </div>
                  <button
                    role="switch"
                    aria-checked={enabled}
                    className={`toggle ${enabled ? 'on' : ''}`}
                    disabled={fixed}
                    onClick={() => void changeOverride(tool, !enabled)}
                  >
                    <span />
                  </button>
                </div>
              );
            })}
          </section>
          <section id="permissions-shortcut" className="settings-panel">
            <div className="panel-heading">
              <div>
                <h3>Parada de emergência</h3>
                <p>
                  Use um atalho global para parar solicitações em andamento. Conflitos aparecem
                  abaixo.
                </p>
              </div>
            </div>
            <div className="preference-row">
              <div>
                <strong>Atalho global</strong>
                <span>Exemplo: Ctrl+Shift+F12</span>
              </div>
              <div className="shortcut-editor">
                <input
                  value={shortcut}
                  onChange={(event) => setShortcut(event.target.value)}
                  aria-label="Atalho global para parar solicitações"
                  maxLength={80}
                />
                <button
                  className="secondary-button small-button"
                  onClick={() => void changeShortcut()}
                >
                  Aplicar
                </button>
              </div>
            </div>
            <div className="preference-row">
              <div>
                <strong>Tempo para aprovar</strong>
                <span>Depois do prazo, a ação é negada automaticamente.</span>
              </div>
              <select
                value={settings.approvalTtlSeconds}
                onChange={(event) => void changeApprovalTtl(Number(event.target.value))}
              >
                <option value={30}>30 segundos</option>
                <option value={60}>1 minuto</option>
                <option value={120}>2 minutos</option>
                <option value={300}>5 minutos</option>
                <option value={600}>10 minutos</option>
              </select>
            </div>
            {settings.shortcutError && <p className="security-error">{settings.shortcutError}</p>}
            <div className="preference-row">
              <div>
                <strong>
                  {settings.halted ? 'O OdeKrai está parado' : 'O OdeKrai está ativo'}
                </strong>
                <span>
                  {settings.halted
                    ? 'Novas ações são recusadas e as respostas em andamento foram canceladas.'
                    : 'Parar cancela solicitações em andamento e nega aprovações pendentes.'}
                </span>
              </div>
              <Button
                variant={settings.halted ? 'primary' : 'danger'}
                onClick={() => void stopOrResume()}
              >
                {settings.halted ? 'Retomar' : 'Parar tudo'}
              </Button>
            </div>
          </section>
          {import.meta.env.DEV && (
            <section className="settings-panel">
              <div className="panel-heading">
                <div>
                  <h3>Testar pedidos de confirmação</h3>
                  <p>
                    Área de teste. As mensagens não são enviadas e pagamentos permanecem bloqueados.
                  </p>
                </div>
              </div>
              <div className="sample-actions">
                <button
                  className="secondary-button"
                  onClick={() =>
                    void runSample({ tool: 'workspace.read_file', path: 'fase3-teste.txt' })
                  }
                >
                  Ler arquivo de teste
                </button>
                <button
                  className="secondary-button"
                  onClick={() =>
                    void runSample({
                      tool: 'workspace.write_file',
                      path: 'fase3-teste.txt',
                      content: 'Arquivo de teste do OdeKrai.',
                    })
                  }
                >
                  Gravar arquivo de teste
                </button>
                <button
                  className="secondary-button"
                  onClick={() =>
                    void runSample({ tool: 'workspace.delete_file', path: 'fase3-teste.txt' })
                  }
                >
                  Mover para a lixeira
                </button>
                <button
                  className="secondary-button"
                  onClick={() =>
                    void runSample({
                      tool: 'simulated.send_message',
                      recipient: 'destinatario@example.invalid',
                      message: 'Mensagem de teste; não enviar.',
                    })
                  }
                >
                  Simular mensagem
                </button>
                <button
                  className="secondary-button"
                  onClick={() =>
                    void runSample({
                      tool: 'simulated.payment',
                      amount_cents: 100,
                      currency: 'USD',
                    })
                  }
                >
                  Testar bloqueio financeiro
                </button>
              </div>
            </section>
          )}
        </>
      )}
      {page === 'activity' && (
        <section className="settings-panel">
          <div className="panel-heading">
            <div>
              <h3>Log de auditoria</h3>
              <p>
                Arquivo JSONL encadeado em {`%USERPROFILE%\\OdeKrai\\logs`}. O log detecta
                alterações, mas não impede um processo local com os mesmos privilégios.
              </p>
            </div>
            <span className={`state-chip ${displayedVerified ? 'connected' : 'warning'}`}>
              {sharedAudit?.unavailable
                ? 'INDISPONÍVEL'
                : displayedVerified === null
                  ? 'VERIFICANDO'
                  : displayedVerified
                    ? 'CADEIA ÍNTEGRA'
                    : 'FALHA NA CADEIA'}
            </span>
            {embedded && onOpenFullPage && (
              <Button type="button" variant="secondary" onClick={onOpenFullPage}>
                Abrir tela completa
              </Button>
            )}
          </div>
          {sharedAudit?.loading && <p>Carregando registros de atividade…</p>}
          <div className="audit-toolbar">
            <input
              value={filter}
              onChange={(event) => setFilter(event.target.value)}
              placeholder="Filtrar ação, origem ou situação"
            />
            <button className="secondary-button small-button" onClick={() => void refresh()}>
              Atualizar
            </button>
            <button
              className="secondary-button small-button"
              disabled={!entries.length}
              onClick={() => {
                const blob = new Blob([entries.map((entry) => JSON.stringify(entry)).join('\n')], {
                  type: 'application/x-ndjson',
                });
                const url = URL.createObjectURL(blob);
                const anchor = document.createElement('a');
                anchor.href = url;
                anchor.download = 'odekrai-atividade.jsonl';
                anchor.click();
                URL.revokeObjectURL(url);
              }}
            >
              Exportar JSONL
            </button>
          </div>
          <div className="audit-list">
            {visibleEntries.map((entry) => (
              <article
                className="audit-entry"
                key={`${entry.id}-${entry.timestamp}-${entry.status}`}
              >
                <div>
                  <strong>{toolLabel(entry.tool)}</strong>
                  <span>
                    {new Date(entry.timestamp * 1000).toLocaleString()} ·{' '}
                    <RiskBadge risk={entry.risk} /> · {entry.origin}
                  </span>
                </div>
                <span className="audit-status">
                  {entry.status} · {entry.decision} ({auditRuleLabel(entry.ruleId)})
                </span>
                <pre>{entry.paramsSummary}</pre>
                {entry.error && <p>{entry.error}</p>}
                <small>Código de integridade: {entry.hash}</small>
              </article>
            ))}
            {!visibleEntries.length && (
              <p className="empty-state">Nenhum evento de auditoria para mostrar.</p>
            )}
          </div>
        </section>
      )}
    </div>
  );
}

export function ApprovalWindow() {
  const [pending, setPending] = useState<Pending | null>(null);
  const [completePreview, setCompletePreview] = useState('');
  const [full, setFull] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const approvalSoundGate = useRef(new SoundTriggerGate());

  async function refresh() {
    try {
      setPending(await invoke<Pending | null>('get_pending_approval'));
      setError('');
    } catch (cause) {
      setError(typeof cause === 'string' ? cause : 'Não foi possível carregar a aprovação.');
    }
  }

  useEffect(() => {
    void refresh();
    let unlisten: (() => void) | undefined;
    let disposed = false;
    void listen('approval-updated', () => void refresh())
      .then((off) => {
        if (disposed) off();
        else unlisten = off;
      })
      .catch(() => setError('Não foi possível acompanhar as aprovações pendentes.'));
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    if (!pending) {
      void getCurrentWindow()
        .close()
        .catch(() =>
          console.warn('[OdeKrai diagnóstico] Não foi possível fechar a janela de aprovação.'),
        );
      return;
    }
    setCompletePreview('');
    setFull(false);
    if (approvalSoundGate.current.claim(pending.id)) {
      void playSound('approval_needed', normalizeSoundPreferences(loadSettings().companion.sounds));
    }
  }, [pending?.id]);

  async function respond(approve: boolean) {
    if (!pending || busy) return;
    setBusy(true);
    try {
      const result = await invoke<{ status: string }>('approval_respond', {
        id: pending.id,
        displayHash: pending.displayHash,
        approve,
      });
      if (result.status === 'completed') {
        void playSound('action_done', normalizeSoundPreferences(loadSettings().companion.sounds));
      }
      await refresh();
    } catch (cause) {
      setError(typeof cause === 'string' ? cause : 'A resposta não foi aplicada.');
      await refresh();
    } finally {
      setBusy(false);
    }
  }

  async function showFullPreview() {
    if (!pending) return;
    try {
      const preview = await invoke<string>('get_full_approval_preview', {
        id: pending.id,
        displayHash: pending.displayHash,
      });
      setCompletePreview(preview);
      setFull(true);
    } catch (cause) {
      setError(typeof cause === 'string' ? cause : 'Não foi possível carregar a prévia completa.');
    }
  }

  const remaining = pending ? Math.max(0, pending.expiresAt - Math.floor(Date.now() / 1000)) : 0;
  useEffect(() => {
    if (pending && remaining === 0) void respond(false);
  }, [remaining]);

  return (
    <main className="approval-window">
      <header>
        <span className="eyebrow">APROVAÇÃO DO ODEKRAI</span>
        <h1>Confirme esta ação</h1>
        <p>Leia destino, origem e prévia. O conteúdo abaixo é texto puro.</p>
      </header>
      {error && <div className="security-notice">{error}</div>}
      {pending ? (
        <>
          <section className="approval-facts">
            <div>
              <small>Ação</small>
              <strong>{toolLabel(pending.tool)}</strong>
            </div>
            <div>
              <small>Risco</small>
              <RiskBadge risk={pending.risk} />
            </div>
            <div>
              <small>Origem</small>
              <strong>{pending.origin}</strong>
            </div>
            <div>
              <small>Ação</small>
              <strong>{pending.summary}</strong>
            </div>
          </section>
          <section className="approval-preview">
            <header>
              <strong>Prévia</strong>
              <span>{remaining}s restantes · expira e é negada automaticamente</span>
            </header>
            <pre>{full ? completePreview : pending.preview}</pre>
            {pending.previewImage && (
              <img
                className="approval-preview-image"
                src={pending.previewImage}
                alt="Prévia reduzida da captura que será salva na pasta de trabalho após aprovação"
              />
            )}
            {pending.fullPreview && (
              <button
                className="text-button"
                onClick={() => (full ? setFull(false) : void showFullPreview())}
              >
                {full ? 'Mostrar trecho' : 'Ver completo'}
              </button>
            )}
          </section>
          <footer>
            <button
              className="secondary-button"
              disabled={busy}
              onClick={() => void respond(false)}
            >
              Negar
            </button>
            <button
              className="primary-button"
              disabled={busy || remaining === 0}
              onClick={() => void respond(true)}
            >
              Aprovar só desta vez
            </button>
          </footer>
        </>
      ) : (
        <p className="empty-state">Nenhuma aprovação pendente. Esta janela será encerrada.</p>
      )}
    </main>
  );
}

export function isApprovalWindow(): boolean {
  try {
    return isTauri() && getCurrentWindow().label === 'approval';
  } catch {
    return false;
  }
}
