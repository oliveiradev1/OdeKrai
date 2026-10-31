import { useCallback, useEffect, useRef, useState, type ReactNode } from 'react';
import { invoke, isTauri } from '@tauri-apps/api/core';
import {
  familyEnableTarget,
  filterSkills,
  groupSkillFamilies,
  nextSkillsTab,
  setSkillEnabledCommand,
  setSkillGroupEnabledCommand,
  setSkillOverrideCommand,
  skillStatusLabel,
  type CatalogFamily,
  type CatalogSkill,
  type DiscordSkillState,
  type SkillFamily,
  type SkillsCatalogTab,
} from '../core/skills-catalog';
import { Button, PageHeader, Toggle } from './DesignSystem';
import { Icon } from './Icon';
import { RiskBadge } from './RiskBadge';
import { PrivacyConfigurationSummary } from './PrivacyConfigurations';

type SkillsCatalogViewProps = {
  skills: CatalogSkill[];
  loading?: boolean;
  busy?: boolean;
  error?: string;
  loadError?: string;
  onRetry?: () => void;
  initialTab?: SkillsCatalogTab;
  initialFamily?: SkillFamily;
  onOpenPrivacy?: () => void;
  onFamilyToggle: (family: SkillFamily, enabled: boolean) => void;
  onSkillToggle: (skill: CatalogSkill, enabled: boolean) => void;
  onOverrideToggle: (skill: CatalogSkill, enabled: boolean) => void;
  googleConnection?: ReactNode;
  prerequisites?: SkillPrerequisites;
};

type GoogleOAuthStatus = {
  connected: boolean;
  clientId: string | null;
  scopes: string[];
  reauthorize: boolean;
};

type SkillPrerequisites = {
  folderAvailable: boolean | null;
  appCount: number | null;
  googleConnected: boolean | null;
};

const skillStatusLabels = {
  installed: 'Ativa',
  available: 'Desativada',
  blocked: 'Bloqueada',
  configuration_required: 'Não configurada',
  unavailable: 'Indisponível',
} as const;

function discordAvailabilityMessage(state?: DiscordSkillState): string {
  switch (state) {
    case 'not_configured':
      return 'Cadastre um webhook para liberar esta habilidade.';
    case 'active':
      return 'Ativa e oferecida ao agente em todas as conversas no modo agente. Cada envio exige aprovação.';
    case 'blocked':
      return 'Bloqueada porque a habilidade está desativada. Ative-a para oferecê-la ao agente.';
    default:
      return 'Não foi possível verificar o estado do Discord.';
  }
}

function statusClass(status: CatalogFamily['status']): string {
  return `skills-family-status--${status}`;
}

export function SkillsCatalogView({
  skills,
  loading = false,
  busy = false,
  error = '',
  loadError = '',
  onRetry,
  initialTab = 'pessoal',
  initialFamily,
  onFamilyToggle,
  onSkillToggle,
  onOverrideToggle,
  onOpenPrivacy,
  googleConnection,
  prerequisites = { folderAvailable: true, appCount: 0, googleConnected: null },
}: SkillsCatalogViewProps) {
  const [query, setQuery] = useState('');
  const [tab, setTab] = useState<SkillsCatalogTab>(initialTab);
  const [selectedFamily, setSelectedFamily] = useState<SkillFamily | null>(initialFamily ?? null);
  const tabRefs = useRef<Record<SkillsCatalogTab, HTMLButtonElement | null>>({
    pessoal: null,
    publico: null,
  });
  const familyButtonRef = useRef<HTMLElement | null>(null);
  const detailsHeadingRef = useRef<HTMLHeadingElement | null>(null);
  const filteredSkills = filterSkills(skills, query);
  const families = groupSkillFamilies(filteredSkills);
  const installedFamilies = groupSkillFamilies(skills).filter((family) => family.activeCount > 0);
  const showFamily = (family: SkillFamily) => {
    familyButtonRef.current =
      document.activeElement instanceof HTMLElement ? document.activeElement : null;
    setTab('pessoal');
    setSelectedFamily(family);
  };

  useEffect(() => {
    if (selectedFamily) detailsHeadingRef.current?.focus();
  }, [selectedFamily]);

  const closeFamily = () => {
    setSelectedFamily(null);
    requestAnimationFrame(() => {
      if (familyButtonRef.current?.isConnected) familyButtonRef.current.focus();
      else document.getElementById('skills-search')?.focus();
    });
  };

  return (
    <div className="page subpage skills-page skills-catalog-page">
      <PageHeader
        eyebrow="CAPACIDADES DO APLICATIVO"
        title="Habilidades"
        subtitle="Veja o que o OdeKrai pode fazer, encontre habilidades e escolha quais ficam ativas."
      />

      {error && (
        <div className="error-banner" role="alert">
          {error}
        </div>
      )}

      {loadError && !loading && (
        <div className="error-banner" role="alert">
          <p>{loadError}</p>
          <Button type="button" onClick={onRetry} disabled={!onRetry}>
            Tentar de novo
          </Button>
        </div>
      )}

      {loading ? (
        <div className="skills-catalog-loading" role="status" aria-live="polite">
          <span className="skills-loading-indicator" aria-hidden="true" />
          Carregando habilidades…
        </div>
      ) : loadError ? null : (
        <>
          <section className="skills-installed" aria-labelledby="skills-installed-heading">
            <div className="skills-catalog-section-heading">
              <div>
                <p className="skills-catalog-eyebrow">ATALHOS</p>
                <h2 id="skills-installed-heading">Instaladas</h2>
              </div>
              <span className="skills-installed-count">
                {installedFamilies.length}{' '}
                {installedFamilies.length === 1 ? 'família ativa' : 'famílias ativas'}
              </span>
            </div>
            {installedFamilies.length > 0 ? (
              <div className="skills-installed-list" aria-label="Famílias ativas">
                {installedFamilies.map((family) => (
                  <button
                    className={`skills-installed-item ${selectedFamily === family.family ? 'selected' : ''}`}
                    key={family.family}
                    type="button"
                    aria-label={`Abrir detalhes de ${family.name}`}
                    aria-expanded={selectedFamily === family.family}
                    onClick={() => showFamily(family.family)}
                  >
                    <Icon name={family.icon} size={18} />
                    <span>{family.name}</span>
                    <span className="skills-installed-item-count">{family.activeCount}</span>
                  </button>
                ))}
              </div>
            ) : (
              <p className="skills-installed-empty">As famílias ativadas aparecerão aqui.</p>
            )}
          </section>

          <div className="skills-tabs" role="tablist" aria-label="Catálogo de habilidades">
            <button
              id="skills-tab-personal"
              className="skills-tab"
              type="button"
              role="tab"
              aria-selected={tab === 'pessoal'}
              aria-controls="skills-panel-personal"
              ref={(element) => {
                tabRefs.current.pessoal = element;
              }}
              onKeyDown={(event) => {
                const next = nextSkillsTab(tab, event.key);
                if (next) {
                  event.preventDefault();
                  setTab(next);
                  tabRefs.current[next]?.focus();
                }
              }}
              onClick={() => setTab('pessoal')}
            >
              Pessoal
            </button>
            <button
              id="skills-tab-public"
              className="skills-tab"
              type="button"
              role="tab"
              aria-selected={tab === 'publico'}
              aria-controls="skills-panel-public"
              ref={(element) => {
                tabRefs.current.publico = element;
              }}
              onKeyDown={(event) => {
                const next = nextSkillsTab(tab, event.key);
                if (next) {
                  event.preventDefault();
                  setTab(next);
                  tabRefs.current[next]?.focus();
                }
              }}
              onClick={() => setTab('publico')}
            >
              Público
            </button>
          </div>

          <section
            id="skills-panel-personal"
            className="skills-tab-panel"
            role="tabpanel"
            aria-labelledby="skills-tab-personal"
            hidden={tab !== 'pessoal'}
            tabIndex={0}
          >
            <div className="skills-search-section">
              <label htmlFor="skills-search">Procurar habilidades</label>
              <input
                id="skills-search"
                type="search"
                value={query}
                onChange={(event) => setQuery(event.target.value)}
                placeholder="Nome, descrição, exemplo ou família"
              />
              <p role="status" aria-live="polite">
                {filteredSkills.length}{' '}
                {filteredSkills.length === 1 ? 'habilidade encontrada' : 'habilidades encontradas'}
              </p>
            </div>

            {selectedFamily ? (
              (() => {
                const selected = groupSkillFamilies(skills).find(
                  (family) => family.family === selectedFamily,
                );
                return selected ? (
                  <SkillFamilyDetails
                    family={selected}
                    busy={busy}
                    prerequisites={prerequisites}
                    headingRef={detailsHeadingRef}
                    onBack={closeFamily}
                    onToggle={(enabled) => onFamilyToggle(selected.family, enabled)}
                    onSkillToggle={onSkillToggle}
                    onOverrideToggle={onOverrideToggle}
                    onOpenPrivacy={onOpenPrivacy}
                    googleConnection={
                      selected.family === 'agenda_lembretes' &&
                      skills.some((skill) => skill.id.startsWith('google.calendar.'))
                        ? googleConnection
                        : undefined
                    }
                  />
                ) : null;
              })()
            ) : families.length === 0 ? (
              <p className="skills-empty" role="status">
                {skills.length === 0
                  ? 'Nenhuma habilidade está registrada nesta versão.'
                  : 'Nenhuma habilidade corresponde à busca.'}
              </p>
            ) : (
              <div className="skills-family-grid">
                {families.map((family) => (
                  <SkillFamilyCard
                    key={family.family}
                    family={family}
                    busy={busy}
                    prerequisites={prerequisites}
                    onExpand={() => showFamily(family.family)}
                    onToggle={(enabled) => onFamilyToggle(family.family, enabled)}
                  />
                ))}
              </div>
            )}
            {onOpenPrivacy && <PrivacyConfigurationSummary onOpen={onOpenPrivacy} />}
          </section>
          <section
            id="skills-panel-public"
            className="skills-tab-panel skills-public-panel"
            role="tabpanel"
            aria-labelledby="skills-tab-public"
            hidden={tab !== 'publico'}
            tabIndex={0}
          >
            <div className="skills-public-empty">
              <Icon name="connectors" size={28} />
              <h2>Plugins públicos em breve</h2>
              <p>O catálogo público ainda não tem plugins publicados.</p>
            </div>
            <div className="skills-public-sections" aria-label="Seções do catálogo público">
              <section aria-labelledby="skills-public-popular">
                <h3 id="skills-public-popular">Populares</h3>
                <p>Nenhum plugin publicado.</p>
              </section>
              <section aria-labelledby="skills-public-new">
                <h3 id="skills-public-new">Novidades</h3>
                <p>Nenhum plugin publicado.</p>
              </section>
              <section aria-labelledby="skills-public-all">
                <h3 id="skills-public-all">Todas as categorias</h3>
                <p>As categorias serão exibidas quando houver plugins publicados.</p>
              </section>
            </div>
          </section>
        </>
      )}
    </div>
  );
}

function SkillFamilyCard({
  family,
  busy,
  prerequisites,
  onExpand,
  onToggle,
}: {
  family: CatalogFamily;
  busy: boolean;
  prerequisites: SkillPrerequisites;
  onExpand: () => void;
  onToggle: (enabled: boolean) => void;
}) {
  const eligible = family.skills.filter(
    (skill) =>
      skill.status === 'available' || skill.status === 'blocked' || skill.status === 'installed',
  );
  const allEnabled = eligible.length > 0 && eligible.every((skill) => skill.enabled);
  const target = familyEnableTarget(family.skills);

  return (
    <article className="skills-family-card">
      <div className="skills-family-card-heading">
        <span className="skills-family-icon" aria-hidden="true">
          <Icon name={family.icon} size={20} />
        </span>
        <div className="skills-family-copy">
          <h2>{family.name}</h2>
          <p>{family.description}</p>
        </div>
        <RiskBadge risk={family.highestRisk} />
      </div>
      <div className="skills-family-card-footer">
        <span className={`skills-family-status ${statusClass(family.status)}`} role="status">
          {skillStatusLabel(family.status)}
        </span>
        <span className="skills-family-count">
          {family.skills.length} {family.skills.length === 1 ? 'habilidade' : 'habilidades'}
        </span>
      </div>
      <div className="skills-family-actions">
        <Button variant="secondary" aria-label={`Gerenciar ${family.name}`} onClick={onExpand}>
          Gerenciar
        </Button>
        <Toggle
          checked={allEnabled}
          label={`${allEnabled ? 'Desativar' : 'Ativar'} todas as habilidades de ${family.name}`}
          disabled={
            busy ||
            target === undefined ||
            (target &&
              family.skills.some((skill) => skill.id.startsWith('google.calendar.')) &&
              prerequisites.googleConnected !== true)
          }
          aria-describedby={`skills-family-${family.family}-description`}
          onClick={() => target !== undefined && onToggle(target)}
        />
      </div>
      <span
        className="skills-family-switch-description"
        id={`skills-family-${family.family}-description`}
      >
        {allEnabled ? 'Desativar família' : 'Ativar família'}.
      </span>
    </article>
  );
}

function SkillFamilyDetails({
  family,
  busy,
  prerequisites,
  headingRef,
  onBack,
  onToggle,
  onSkillToggle,
  onOverrideToggle,
  onOpenPrivacy,
  googleConnection,
}: {
  family: CatalogFamily;
  busy: boolean;
  prerequisites: SkillPrerequisites;
  headingRef: React.RefObject<HTMLHeadingElement | null>;
  onBack: () => void;
  onToggle: (enabled: boolean) => void;
  onSkillToggle: SkillsCatalogViewProps['onSkillToggle'];
  onOverrideToggle: SkillsCatalogViewProps['onOverrideToggle'];
  onOpenPrivacy?: () => void;
  googleConnection?: ReactNode;
}) {
  const eligible = family.skills.filter(
    (skill) =>
      skill.status === 'available' || skill.status === 'blocked' || skill.status === 'installed',
  );
  const allEnabled = eligible.length > 0 && eligible.every((skill) => skill.enabled);
  const hasConfigurationRequirement = family.skills.some((skill) => skill.configurationRequired);
  const requiresGoogle = family.skills.some((skill) => skill.id.startsWith('google.calendar.'));
  const needsGoogleConnection = requiresGoogle && prerequisites.googleConnected === false;
  const requiresApps = family.skills.some((skill) => skill.id === 'app.launch');
  const requiresFolder = family.skills.some((skill) =>
    /^(files\.|workspace\.|doc\.|wallpaper\.)/.test(skill.id),
  );
  const origins = [
    ...new Set(
      family.skills.map((skill) =>
        skill.origin.kind === 'builtin' ? 'OdeKrai' : `Servidor MCP: ${skill.origin.server}`,
      ),
    ),
  ];
  const categories = [
    ...new Set(family.skills.map((skill) => (skill.category === 'local' ? 'Local' : 'Integração'))),
  ];
  const hasUnconfiguredApps = requiresApps && prerequisites.appCount === 0;
  const needsConfiguration =
    hasConfigurationRequirement || hasUnconfiguredApps || needsGoogleConnection;

  return (
    <section className="skills-family-details-page" aria-labelledby="skills-family-detail-title">
      <Button variant="ghost" onClick={onBack} className="skills-details-back">
        Voltar às habilidades
      </Button>
      <header className="skills-details-heading">
        <span className="skills-family-icon" aria-hidden="true">
          <Icon name={family.icon} size={24} />
        </span>
        <div>
          <p className="skills-catalog-eyebrow">DETALHES DA FAMÍLIA</p>
          <h2 id="skills-family-detail-title" ref={headingRef} tabIndex={-1}>
            {family.name}
          </h2>
          <p>{family.description}</p>
        </div>
        <RiskBadge risk={family.highestRisk} />
      </header>
      <dl className="skills-details-metadata">
        <div>
          <dt>Categoria</dt>
          <dd>{categories.join(' e ')}</dd>
        </div>
        <div>
          <dt>Origem</dt>
          <dd>{origins.join(' · ')}</dd>
        </div>
        <div>
          <dt>Estado</dt>
          <dd>
            <span className={`skills-family-status ${statusClass(family.status)}`}>
              {skillStatusLabel(family.status)}
            </span>
          </dd>
        </div>
      </dl>
      <div className="skills-details-primary-action">
        {needsConfiguration || hasUnconfiguredApps ? (
          <Button
            variant="primary"
            disabled={!needsGoogleConnection && !onOpenPrivacy}
            onClick={() => {
              if (needsGoogleConnection) document.getElementById('google-client-id')?.focus();
              else onOpenPrivacy?.();
            }}
          >
            {needsGoogleConnection || hasConfigurationRequirement
              ? 'Conectar'
              : 'Cadastrar aplicativo'}
          </Button>
        ) : (
          <Button
            variant="primary"
            disabled={busy || eligible.length === 0}
            onClick={() => onToggle(!allEnabled)}
          >
            {allEnabled ? 'Desativar família' : 'Ativar família'}
          </Button>
        )}
      </div>
      <section className="skills-prerequisites" aria-labelledby="skills-prerequisites-title">
        <h3 id="skills-prerequisites-title">Requisitos e estado</h3>
        {requiresFolder && (
          <p>
            <strong>Pasta permitida:</strong>{' '}
            {prerequisites.folderAvailable === null
              ? 'não foi possível consultar o estado.'
              : prerequisites.folderAvailable
                ? 'pasta de trabalho disponível; pastas adicionais podem ser permitidas.'
                : 'nenhuma pasta disponível.'}
          </p>
        )}
        {requiresApps && (
          <p>
            <strong>Aplicativo cadastrado:</strong>{' '}
            {prerequisites.appCount === null
              ? 'não foi possível consultar o estado.'
              : prerequisites.appCount > 0
                ? `${prerequisites.appCount} aplicativo(s) disponível(is).`
                : 'nenhum aplicativo cadastrado.'}
          </p>
        )}
        {hasConfigurationRequirement && (
          <p>
            <strong>Conexão necessária:</strong>{' '}
            {family.skills
              .filter((skill) => skill.configurationRequired)
              .map((skill) => skill.name)
              .join(', ')}{' '}
            ainda precisa de configuração.
          </p>
        )}
        {requiresGoogle && (
          <p>
            <strong>Conta Google:</strong>{' '}
            {prerequisites.googleConnected === null
              ? 'estado não verificado.'
              : prerequisites.googleConnected
                ? 'conectada.'
                : 'desconectada.'}
          </p>
        )}
        {!requiresFolder && !requiresApps && !hasConfigurationRequirement && !requiresGoogle && (
          <p>Não há configuração adicional registrada para esta família.</p>
        )}
        {onOpenPrivacy &&
          (requiresFolder || requiresApps || hasConfigurationRequirement || requiresGoogle) && (
            <Button variant="secondary" onClick={onOpenPrivacy}>
              Abrir Privacidade e permissões
            </Button>
          )}
      </section>
      {googleConnection}
      <section className="skills-detail-list" aria-labelledby="skills-detail-list-title">
        <h3 id="skills-detail-list-title">Habilidades desta família</h3>
        {family.skills.map((skill) => (
          <article className="skills-detail-item" key={skill.id}>
            <div className="skills-detail-copy">
              <div className="skills-detail-title">
                <h4>{skill.name}</h4>
                <span className={`skills-detail-status skills-detail-status--${skill.status}`}>
                  {skillStatusLabels[skill.status]}
                </span>
                {skill.description.toLocaleLowerCase('pt-BR').includes('experimental') && (
                  <span className="skills-experimental-badge">Experimental</span>
                )}
              </div>
              <p>{skill.description}</p>
              {skill.status === 'unavailable' && skill.statusReason && (
                <p className="skills-tool-exposure" role="status">
                  Motivo: {skill.statusReason}
                </p>
              )}
              {skill.id === 'discord.send' && (
                <>
                  <p className="skills-tool-exposure" role="status">
                    <strong>Estado do Discord:</strong>{' '}
                    {discordAvailabilityMessage(skill.discordState)}
                  </p>
                  {skill.discordState === 'not_configured' && onOpenPrivacy && (
                    <Button variant="secondary" onClick={onOpenPrivacy}>
                      Cadastrar webhook
                    </Button>
                  )}
                </>
              )}
              {skill.examples.length > 0 && (
                <ul aria-label={`Exemplos de pedidos para ${skill.name}`}>
                  {skill.examples.map((example) => (
                    <li key={example}>{example}</li>
                  ))}
                </ul>
              )}
              <span className="skills-detail-origin">
                {skill.origin.kind === 'builtin'
                  ? 'Origem: OdeKrai'
                  : `Servidor MCP: ${skill.origin.server}`}
              </span>
              <RiskBadge risk={skill.risk} />
            </div>
            <div className="skills-detail-controls">
              <Toggle
                checked={skill.enabled}
                label={`${skill.enabled ? 'Desativar' : 'Ativar'} ${skill.name}`}
                disabled={
                  busy ||
                  skill.status === 'unavailable' ||
                  skill.status === 'configuration_required' ||
                  (skill.id.startsWith('google.calendar.') &&
                    prerequisites.googleConnected !== true)
                }
                onClick={() => onSkillToggle(skill, !skill.enabled)}
              />
            </div>
            {skill.fixedBlocked && (
              <p className="skills-fixed-blocked" role="note">
                Bloqueio fixo: esta habilidade não pode ser liberada por um modo de permissão.
              </p>
            )}
            {skill.id === 'discord.send' && skill.discordState === 'active' && (
              <p className="skills-critical-approval-note" role="note">
                Cada envio exige aprovação na janela nativa, em qualquer modo de permissão.
              </p>
            )}
            {!skill.fixedBlocked &&
              skill.id !== 'discord.send' &&
              skill.status !== 'unavailable' && (
                <details className="skill-advanced skills-override-settings">
                  <summary>Gerenciar</summary>
                  <p>
                    Esta exceção permite usar menos confirmações que o modo global. O OdeKrai
                    solicitará confirmação nativa antes de salvar.
                  </p>
                  <Toggle
                    checked={skill.morePermissive}
                    label={`${skill.morePermissive ? 'Remover' : 'Permitir'} modo mais permissivo para ${skill.name}`}
                    disabled={busy}
                    onClick={() => onOverrideToggle(skill, !skill.morePermissive)}
                  />
                </details>
              )}
          </article>
        ))}
      </section>
      <p className="skills-critical-approval-note" role="note">
        Controle total não remove aprovações críticas. Pagamentos, comunicação externa e ações
        bloqueadas continuam sob as proteções do OdeKrai.
      </p>
    </section>
  );
}

export function SkillsScreen({
  onOpenPrivacy,
  initialFamily,
}: {
  onOpenPrivacy: () => void;
  initialFamily?: SkillFamily;
}) {
  const [skills, setSkills] = useState<CatalogSkill[]>([]);
  const [error, setError] = useState('');
  const [loadError, setLoadError] = useState('');
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [googleStatus, setGoogleStatus] = useState<GoogleOAuthStatus | null>(null);
  const [prerequisites, setPrerequisites] = useState<SkillPrerequisites>({
    folderAvailable: true,
    appCount: 0,
    googleConnected: null,
  });

  const refresh = useCallback(async (silent = false) => {
    if (!isTauri()) {
      setSkills([]);
      setLoadError('As habilidades só podem ser carregadas no aplicativo OdeKrai.');
      setLoading(false);
      return;
    }
    if (!silent) setLoading(true);
    try {
      const nextSkills = await invoke<CatalogSkill[]>('get_skills');
      setSkills(nextSkills);
      const [folders, apps] = await Promise.all([
        invoke<Array<{ workspace: boolean }>>('get_allowed_folders').catch(() => null),
        invoke<Array<{ id: string }>>('get_allowed_apps').catch(() => null),
      ]);
      setPrerequisites({
        folderAvailable: folders === null ? null : folders.some((folder) => folder.workspace),
        appCount: apps === null ? null : apps.length,
        googleConnected: null,
      });
      if (nextSkills.some((skill) => skill.id.startsWith('google.calendar.'))) {
        const status = await invoke<GoogleOAuthStatus>('google_oauth_status').catch(() => null);
        setGoogleStatus(status);
        setPrerequisites((current) => ({ ...current, googleConnected: status?.connected ?? null }));
      } else {
        setGoogleStatus(null);
        setPrerequisites((current) => ({ ...current, googleConnected: null }));
      }
      setLoadError('');
      setError('');
    } catch (cause) {
      setSkills([]);
      setLoadError(`Não foi possível carregar as habilidades. ${String(cause)}`);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  async function runCommand(action: () => Promise<void>) {
    setBusy(true);
    try {
      await action();
      await refresh(true);
    } catch (cause) {
      setError(String(cause));
    } finally {
      setBusy(false);
    }
  }

  return (
    <SkillsCatalogView
      skills={skills}
      loading={loading}
      busy={busy}
      error={error}
      loadError={loadError}
      onRetry={() => void refresh()}
      onOpenPrivacy={onOpenPrivacy}
      googleConnection={
        googleStatus ? (
          <GoogleCalendarConnection
            status={googleStatus}
            onStatusChange={(status) => {
              setGoogleStatus(status);
              setPrerequisites((current) => ({ ...current, googleConnected: status.connected }));
            }}
            onError={setError}
          />
        ) : undefined
      }
      prerequisites={prerequisites}
      initialFamily={initialFamily}
      onFamilyToggle={(family, enabled) =>
        void runCommand(() => setSkillGroupEnabledCommand(family, enabled, invoke))
      }
      onSkillToggle={(skill, enabled) =>
        void runCommand(() => setSkillEnabledCommand(skill.id, enabled, invoke))
      }
      onOverrideToggle={(skill, enabled) =>
        void runCommand(() => setSkillOverrideCommand(skill.id, enabled, invoke))
      }
    />
  );
}

function GoogleCalendarConnection({
  status,
  onStatusChange,
  onError,
}: {
  status: GoogleOAuthStatus;
  onStatusChange: (status: GoogleOAuthStatus) => void;
  onError: (message: string) => void;
}) {
  const [busy, setBusy] = useState(false);
  const [clientId, setClientId] = useState(status.clientId ?? '');
  const [clientSecret, setClientSecret] = useState('');
  const [scopes, setScopes] = useState('');

  return (
    <section className="skills-google-connection" aria-labelledby="skills-google-heading">
      <div className="skills-google-heading">
        <div>
          <h4 id="skills-google-heading">Conexão com o Google</h4>
          <p>Necessária para usar as habilidades do Google Calendar.</p>
        </div>
        <span
          className={`skills-family-status ${status.connected ? 'skills-family-status--active' : 'skills-family-status--configuration_required'}`}
          role="status"
        >
          {status.connected ? 'Conectado' : 'Desconectado'}
        </span>
      </div>
      {status.connected ? (
        <p className="skills-google-scopes">
          Identificador: <code>{status.clientId}</code>
          <br />
          Acessos autorizados: {status.scopes.join(', ')}
        </p>
      ) : (
        <form
          className="skills-google-fields"
          onSubmit={(event) => {
            event.preventDefault();
            setBusy(true);
            void invoke<GoogleOAuthStatus>('google_oauth_connect', {
              clientId,
              clientSecret: clientSecret || null,
              scopes: scopes.trim().split(/\s+/),
            })
              .then((nextStatus) => {
                onStatusChange(nextStatus);
                setClientSecret('');
              })
              .catch((cause) => onError(String(cause)))
              .finally(() => setBusy(false));
          }}
        >
          <label htmlFor="google-client-id">Identificador do cliente Google</label>
          <input
            id="google-client-id"
            value={clientId}
            onChange={(event) => setClientId(event.target.value)}
            autoComplete="off"
          />
          <label htmlFor="google-client-secret">Chave opcional do cliente Google</label>
          <input
            id="google-client-secret"
            type="password"
            value={clientSecret}
            onChange={(event) => setClientSecret(event.target.value)}
            autoComplete="new-password"
          />
          <label htmlFor="google-oauth-scopes">
            Acessos solicitados pelo recurso que você vai usar
          </label>
          <input
            id="google-oauth-scopes"
            value={scopes}
            onChange={(event) => setScopes(event.target.value)}
            placeholder="https://www.googleapis.com/auth/calendar.events.owned"
          />
          <p>
            Para a Agenda Google, informe somente
            https://www.googleapis.com/auth/calendar.events.owned. Aplicativos de computador não
            conseguem manter uma chave de cliente em segredo.
          </p>
          <Button type="submit" disabled={busy || !isTauri() || !clientId.trim() || !scopes.trim()}>
            Conectar conta Google
          </Button>
        </form>
      )}
      {status.connected && (
        <Button
          variant="secondary"
          disabled={busy || !isTauri()}
          onClick={() => {
            setBusy(true);
            void invoke('google_oauth_disconnect')
              .then(() => {
                onStatusChange({
                  connected: false,
                  clientId: null,
                  scopes: [],
                  reauthorize: false,
                });
                setClientSecret('');
              })
              .catch(async (cause) => {
                onError(String(cause));
                try {
                  onStatusChange(await invoke<GoogleOAuthStatus>('google_oauth_status'));
                } catch {
                  // Mantém o último estado conhecido se a leitura também falhar.
                }
              })
              .finally(() => setBusy(false));
          }}
        >
          Desconectar
        </Button>
      )}
    </section>
  );
}
