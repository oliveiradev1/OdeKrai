import {
  useCallback,
  useEffect,
  useId,
  useRef,
  useState,
  type FormEvent,
  type ReactNode,
} from 'react';
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
import { submitSecretForm } from './secret-form';
import { RiskBadge } from './RiskBadge';
import { PrivacyConfigurationSummary } from './PrivacyConfigurations';
import {
  canConnectToPublicOAuth,
  canConnectWithOAuth,
  getPublicMcpStatus,
  publicOAuthServiceKey,
  publicMcpCatalog,
  publicMcpStatusLabels,
  type PublicMcpCatalogEntry,
  type OAuthRegistrationOptions,
  type PublicOAuthClientSettings,
  type PublicMcpServer,
} from '../core/mcp-service-catalog';

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
  mcpServers?: Array<PublicMcpServer & { id: string }>;
  githubOAuthAccount?: string | null;
  mcpBusy?: boolean;
  onConfigurePublicMcp?: (
    service: PublicMcpCatalogEntry,
    endpoint: string,
    authentication: 'oauth' | 'bearer',
    token: string,
    clientId: string,
    scopes: string,
  ) => Promise<boolean>;
  onSaveGitHubToken?: (server: PublicMcpServer & { id: string }, token: string) => Promise<boolean>;
  onRemoveGitHubToken?: (server: PublicMcpServer & { id: string }) => Promise<boolean>;
  onConnectGitHubOAuth?: (
    server: (PublicMcpServer & { id: string }) | undefined,
    clientId: string,
    scopes: string,
  ) => Promise<boolean>;
  onDisconnectGitHubOAuth?: (server: PublicMcpServer & { id: string }) => Promise<string | null>;
  onTestPublicMcp?: (server: PublicMcpServer & { id: string }) => Promise<boolean>;
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
  mcpServers = [],
  githubOAuthAccount,
  mcpBusy = false,
  onConfigurePublicMcp,
  onSaveGitHubToken,
  onRemoveGitHubToken,
  onConnectGitHubOAuth,
  onDisconnectGitHubOAuth,
  onTestPublicMcp,
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
                      selected.family === 'google' &&
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
              <h2>Servidores MCP públicos</h2>
              <p>
                Consulte os servidores oficiais. Um serviço só aparece como conectado depois de um
                teste de conexão bem-sucedido.
              </p>
            </div>
            <div className="public-mcp-grid" aria-label="Catálogo de serviços MCP">
              {publicMcpCatalog.map((service) => {
                const server = mcpServers.find(
                  (item) => item.endpoint === service.endpoint || item.name === service.name,
                );
                return (
                  <PublicMcpServiceCard
                    key={service.name}
                    service={service}
                    status={getPublicMcpStatus(service, mcpServers)}
                    server={server}
                    githubOAuthAccount={githubOAuthAccount}
                    busy={mcpBusy}
                    onConfigure={onConfigurePublicMcp}
                    onSaveGitHubToken={onSaveGitHubToken}
                    onRemoveGitHubToken={onRemoveGitHubToken}
                    onConnectGitHubOAuth={onConnectGitHubOAuth}
                    onDisconnectGitHubOAuth={onDisconnectGitHubOAuth}
                    onTest={onTestPublicMcp}
                  />
                );
              })}
            </div>
          </section>
        </>
      )}
    </div>
  );
}

function PublicMcpServiceCard({
  service,
  status,
  server,
  githubOAuthAccount: initialGitHubAccount,
  busy,
  onConfigure,
  onSaveGitHubToken,
  onRemoveGitHubToken,
  onConnectGitHubOAuth,
  onDisconnectGitHubOAuth,
  onTest,
}: {
  service: PublicMcpCatalogEntry;
  status: ReturnType<typeof getPublicMcpStatus>;
  server?: PublicMcpServer & { id: string };
  githubOAuthAccount?: string | null;
  busy: boolean;
  onConfigure?: SkillsCatalogViewProps['onConfigurePublicMcp'];
  onSaveGitHubToken?: SkillsCatalogViewProps['onSaveGitHubToken'];
  onRemoveGitHubToken?: SkillsCatalogViewProps['onRemoveGitHubToken'];
  onConnectGitHubOAuth?: SkillsCatalogViewProps['onConnectGitHubOAuth'];
  onDisconnectGitHubOAuth?: SkillsCatalogViewProps['onDisconnectGitHubOAuth'];
  onTest?: SkillsCatalogViewProps['onTestPublicMcp'];
}) {
  const formId = useId();
  const [expanded, setExpanded] = useState(false);
  const [endpoint, setEndpoint] = useState(service.endpoint);
  const [authentication, setAuthentication] = useState<'oauth' | 'bearer'>(
    (service.name === 'GitHub'
      ? 'bearer'
      : (service.authenticationMethods[0] as 'oauth' | 'bearer' | undefined)) ?? 'oauth',
  );
  const [token, setToken] = useState('');
  const [clientId, setClientId] = useState('');
  const [oauthClientSettings, setOauthClientSettings] = useState<PublicOAuthClientSettings | null>(
    null,
  );
  const [clientSecret, setClientSecret] = useState('');
  const [clientSecretBusy, setClientSecretBusy] = useState(false);
  const [scopes, setScopes] = useState('');
  const [registrationOptions, setRegistrationOptions] = useState<OAuthRegistrationOptions | null>(
    null,
  );
  const [checkingOAuth, setCheckingOAuth] = useState(false);
  const [oauthError, setOauthError] = useState('');
  const [linkError, setLinkError] = useState('');
  const [githubAccount, setGithubAccount] = useState<string | null>(initialGitHubAccount ?? null);
  const [githubAccountLoading, setGithubAccountLoading] = useState(false);
  const [githubNotice, setGithubNotice] = useState('');
  const isGitHub = service.name === 'GitHub';
  const isGoogleWorkspace = service.supplier === 'Google Workspace';
  const githubOAuthReady = canConnectToPublicOAuth(
    oauthClientSettings,
    { dynamicRegistration: false },
    busy || clientSecretBusy,
  );
  const publicOAuthReady = canConnectToPublicOAuth(
    oauthClientSettings,
    registrationOptions,
    busy || clientSecretBusy,
  );

  useEffect(() => {
    if (!service.authenticationMethods.includes('oauth') || !isTauri()) return;
    let current = true;
    void invoke<PublicOAuthClientSettings>('mcp_oauth_public_client_settings', {
      service: publicOAuthServiceKey(service),
    })
      .then((settings) => {
        if (!current) return;
        setOauthClientSettings(settings);
        setClientId(settings.clientId);
        if (service.supplier === 'Google Workspace') {
          setRegistrationOptions({ dynamicRegistration: false });
        }
      })
      .catch((cause) => {
        if (current) setOauthError(String(cause));
      });
    return () => {
      current = false;
    };
  }, [service]);

  useEffect(() => {
    if (!isGitHub || !server || status !== 'connected' || !isTauri()) {
      setGithubAccount(null);
      return;
    }
    let current = true;
    setGithubAccountLoading(true);
    void invoke<string | null>('mcp_oauth_github_account', { serverId: server.id })
      .then((account) => {
        if (current) setGithubAccount(account);
      })
      .catch((cause) => {
        if (current) setGithubNotice(`Não foi possível consultar a conta GitHub: ${String(cause)}`);
      })
      .finally(() => {
        if (current) setGithubAccountLoading(false);
      });
    return () => {
      current = false;
    };
  }, [isGitHub, server?.id, status]);

  async function saveOAuthClientSecret() {
    if (!oauthClientSettings || !clientSecret.trim() || !isTauri()) return;
    setClientSecretBusy(true);
    setOauthError('');
    try {
      await invoke('mcp_oauth_save_public_client_secret', {
        service: oauthClientSettings.service,
        secret: clientSecret,
      });
      setClientSecret('');
      setOauthClientSettings({ ...oauthClientSettings, clientSecretSaved: true });
    } catch (cause) {
      setOauthError(String(cause));
    } finally {
      setClientSecretBusy(false);
    }
  }

  async function removeOAuthClientSecret() {
    if (!oauthClientSettings || !isTauri()) return;
    setClientSecretBusy(true);
    setOauthError('');
    try {
      await invoke('mcp_oauth_remove_public_client_secret', {
        service: oauthClientSettings.service,
      });
      setOauthClientSettings({ ...oauthClientSettings, clientSecretSaved: false });
      setClientSecret('');
    } catch (cause) {
      setOauthError(String(cause));
    } finally {
      setClientSecretBusy(false);
    }
  }

  async function openOfficialLink(url: string) {
    if (!isTauri()) return;
    try {
      await invoke('open_external_link', { url });
    } catch (cause) {
      setLinkError(String(cause));
    }
  }

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!onConfigure) return;
    if (authentication === 'oauth' && !canConnectWithOAuth(registrationOptions, clientId, busy)) {
      return;
    }
    try {
      if (await onConfigure(service, endpoint, authentication, token, clientId, scopes)) {
        setClientId('');
        setScopes('');
        setExpanded(false);
      }
    } finally {
      if (authentication === 'bearer') setToken('');
    }
  }

  async function checkOAuthRegistration() {
    if (!isTauri()) {
      setOauthError('A verificação das opções OAuth só está disponível no aplicativo OdeKrai.');
      return;
    }
    setCheckingOAuth(true);
    setOauthError('');
    setRegistrationOptions(null);
    try {
      const options = await invoke<OAuthRegistrationOptions>('mcp_oauth_registration_options', {
        endpoint,
      });
      setRegistrationOptions(options);
    } catch (cause) {
      setOauthError(String(cause));
    } finally {
      setCheckingOAuth(false);
    }
  }

  async function connectGitHubOAuth() {
    if (!oauthClientSettings || !onConnectGitHubOAuth || !githubOAuthReady) return;
    setGithubNotice('');
    try {
      await onConnectGitHubOAuth(server, oauthClientSettings.clientId, 'read:user');
    } catch (cause) {
      setGithubNotice(`Não foi possível conectar ao GitHub: ${String(cause)}`);
    }
  }

  async function disconnectGitHubOAuth() {
    if (!server || !onDisconnectGitHubOAuth) return;
    setGithubNotice('');
    try {
      const warning = await onDisconnectGitHubOAuth(server);
      setGithubAccount(null);
      setGithubNotice(
        warning ??
          'Conta desconectada. As credenciais locais foram removidas do Gerenciador de Credenciais.',
      );
    } catch (cause) {
      setGithubNotice(`Não foi possível desconectar a conta GitHub: ${String(cause)}`);
    }
  }

  return (
    <article className="public-mcp-card">
      <div className="public-mcp-card-heading">
        <div>
          <h3>{service.name}</h3>
          <p>{service.supplier}</p>
        </div>
        <span className={`public-mcp-status public-mcp-status--${status}`} role="status">
          {publicMcpStatusLabels[status]}
        </span>
      </div>
      {service.programStatus && (
        <p className="public-mcp-program-status">{service.programStatus}</p>
      )}
      {isGoogleWorkspace && (
        <section className="public-mcp-google-setup" aria-label={`Configuração de ${service.name}`}>
          <h4>Como preparar o projeto Google</h4>
          <ul>
            {service.oauthScopeLabels?.map((scope) => (
              <li key={scope}>{scope}</li>
            ))}
          </ul>
          <ol>
            {service.oauthSetupSteps?.map((step) => (
              <li key={step}>{step}</li>
            ))}
          </ol>
          <Button
            type="button"
            variant="secondary"
            onClick={() => void openOfficialLink(service.documentationUrl)}
          >
            Abrir guia oficial do {service.name}
          </Button>
          {linkError && <p role="alert">{linkError}</p>}
        </section>
      )}
      {isGitHub && (
        <div className="public-mcp-github-oauth">
          <p>
            O OdeKrai pedirá somente <code>read:user</code> para identificar a conta. A conexão do
            servidor fica em modo somente leitura; não são solicitadas permissões de escrita.
            Confira as permissões exibidas pelo GitHub antes de autorizar. Sem permissões de
            repositório, dados privados podem não ficar disponíveis.
          </p>
          {status === 'connected' && githubAccount && (
            <p className="public-mcp-connected" role="status">
              Conectado como <strong>@{githubAccount}</strong>
            </p>
          )}
          {status === 'connected' && githubAccountLoading && (
            <p role="status">Verificando conta…</p>
          )}
          {status === 'connected' && githubAccount && onDisconnectGitHubOAuth && (
            <Button
              type="button"
              variant="secondary"
              disabled={busy}
              onClick={() => void disconnectGitHubOAuth()}
            >
              Desconectar
            </Button>
          )}
          {status !== 'connected' || (!githubAccount && !githubAccountLoading) ? (
            <Button
              type="button"
              disabled={!githubOAuthReady || busy || !onConnectGitHubOAuth}
              onClick={() => void connectGitHubOAuth()}
            >
              Conectar com GitHub
            </Button>
          ) : null}
          {oauthClientSettings && !oauthClientSettings.clientId.trim() && (
            <p role="status">
              Este aplicativo ainda precisa de um OAuth App GitHub registrado pelo desenvolvedor.{' '}
              <Button
                type="button"
                variant="secondary"
                onClick={() => void openOfficialLink(oauthClientSettings.guideUrl)}
              >
                Abrir guia oficial
              </Button>
            </p>
          )}
          {oauthClientSettings?.clientSecretRequired && oauthClientSettings.clientId.trim() && (
            <form
              className="public-mcp-oauth-secret-form"
              onSubmit={(event) => {
                event.preventDefault();
                void saveOAuthClientSecret();
              }}
            >
              <label>
                Segredo do OAuth App do GitHub
                <input
                  id={`${formId}-github-oauth-client-secret`}
                  name={`${formId}-github-oauth-client-secret`}
                  type="password"
                  autoComplete="off"
                  value={clientSecret}
                  onChange={(event) => setClientSecret(event.target.value)}
                />
                <small>
                  O GitHub exige o segredo para trocar o código OAuth. Ele fica no Gerenciador de
                  Credenciais e não volta a ser exibido. Use um app dedicado ao OdeKrai; o segredo
                  fica acessível a quem usa este dispositivo.
                </small>
              </label>
              {oauthClientSettings.clientSecretSaved ? (
                <>
                  <span role="status">Segredo salvo</span>
                  <Button
                    type="button"
                    variant="secondary"
                    disabled={clientSecretBusy}
                    onClick={() => void removeOAuthClientSecret()}
                  >
                    Remover segredo
                  </Button>
                  {clientSecret.trim() && (
                    <Button type="submit" variant="secondary" disabled={clientSecretBusy}>
                      Substituir segredo
                    </Button>
                  )}
                </>
              ) : (
                <Button
                  type="submit"
                  variant="secondary"
                  disabled={clientSecretBusy || !clientSecret.trim()}
                >
                  Salvar segredo
                </Button>
              )}
            </form>
          )}
          {githubNotice && (
            <p role={githubNotice.startsWith('Não foi possível') ? 'alert' : 'status'}>
              {githubNotice}
            </p>
          )}
        </div>
      )}
      {service.authenticationMethods.includes('oauth') && !oauthClientSettings && oauthError && (
        <p role="alert">{oauthError}</p>
      )}
      {service.authenticationMethods.includes('oauth') &&
        !isGitHub &&
        oauthClientSettings &&
        endpoint === service.endpoint && (
          <div className="public-mcp-oauth-client-setup">
            {!oauthClientSettings.clientId.trim() &&
              (!oauthClientSettings.dynamicRegistration ||
                registrationOptions?.dynamicRegistration === false) && (
                <p role="status">
                  Este serviço ainda não foi configurado para o OdeKrai. Veja o guia ({' '}
                  <Button
                    type="button"
                    variant="secondary"
                    onClick={() => void openOfficialLink(oauthClientSettings.guideUrl)}
                  >
                    abrir documentação oficial
                  </Button>
                  ).
                </p>
              )}
            {oauthClientSettings.clientSecretRequired &&
              (oauthClientSettings.clientId.trim() || oauthClientSettings.clientSecretSaved) && (
                <form
                  className="public-mcp-oauth-secret-form"
                  onSubmit={(event) => {
                    event.preventDefault();
                    void saveOAuthClientSecret();
                  }}
                >
                  <label>
                    Segredo do cliente OAuth
                    <input
                      id={`${formId}-oauth-client-secret`}
                      name={`${formId}-oauth-client-secret`}
                      type="password"
                      autoComplete="off"
                      value={clientSecret}
                      disabled={!oauthClientSettings.clientId.trim()}
                      onChange={(event) => setClientSecret(event.target.value)}
                    />
                    <small>
                      O segredo fica no Gerenciador de Credenciais e não volta a ser exibido.
                    </small>
                  </label>
                  {oauthClientSettings.clientSecretSaved ? (
                    <>
                      <span role="status">Segredo salvo</span>
                      <Button
                        type="button"
                        variant="secondary"
                        disabled={clientSecretBusy}
                        onClick={() => void removeOAuthClientSecret()}
                      >
                        Remover segredo
                      </Button>
                      {oauthClientSettings.clientId.trim() && clientSecret.trim() && (
                        <Button type="submit" variant="secondary" disabled={clientSecretBusy}>
                          Substituir segredo
                        </Button>
                      )}
                    </>
                  ) : (
                    <Button
                      type="submit"
                      variant="secondary"
                      disabled={
                        clientSecretBusy ||
                        !clientSecret.trim() ||
                        !oauthClientSettings.clientId.trim()
                      }
                    >
                      Salvar segredo
                    </Button>
                  )}
                </form>
              )}
            {isGoogleWorkspace &&
              oauthClientSettings.clientId.trim() &&
              !oauthClientSettings.clientSecretSaved && (
                <p role="alert">
                  Salve o segredo OAuth deste serviço no Gerenciador de Credenciais antes de
                  conectar.
                </p>
              )}
            {!oauthClientSettings.clientId.trim() && !oauthClientSettings.dynamicRegistration && (
              <Button type="button" variant="secondary" disabled>
                Conectar
              </Button>
            )}
            {oauthError && <p role="alert">{oauthError}</p>}
          </div>
        )}
      <dl>
        <div>
          <dt>Endereço oficial</dt>
          <dd>{service.endpoint}</dd>
        </div>
        <div>
          <dt>Transporte</dt>
          <dd>{service.transport}</dd>
        </div>
        <div>
          <dt>Autenticação</dt>
          <dd>{service.authentication}</dd>
        </div>
      </dl>
      {service.tokenCreationUrl && service.tokenPermissions && (
        <div className="public-mcp-token-guide">
          <p>Crie um token pessoal fine-grained no GitHub. Sugestão mínima para leitura:</p>
          <ul>
            {service.tokenPermissions.map((permission) => (
              <li key={permission}>{permission}</li>
            ))}
          </ul>
          <Button
            type="button"
            variant="secondary"
            onClick={() => void openOfficialLink(service.tokenCreationUrl!)}
          >
            Criar token pessoal no GitHub
          </Button>
          {linkError && <p role="alert">{linkError}</p>}
        </div>
      )}
      {status === 'not_available' && (
        <p className="public-mcp-unavailable">
          Este serviço ainda não pode ser configurado no OdeKrai por uma limitação de
          compatibilidade.
        </p>
      )}
      {isGitHub && (
        <Button
          type="button"
          variant="secondary"
          disabled={busy}
          aria-expanded={expanded}
          aria-controls={formId}
          onClick={() => {
            setAuthentication('bearer');
            setExpanded((open) => !open);
          }}
        >
          {expanded ? 'Fechar opção avançada' : 'Usar token pessoal (avançado)'}
        </Button>
      )}
      {!isGitHub && status === 'available' && onConfigure && (
        <Button
          type="button"
          variant="secondary"
          disabled={busy}
          aria-expanded={expanded}
          aria-controls={formId}
          onClick={() => setExpanded((open) => !open)}
        >
          Configurar
        </Button>
      )}
      {isGoogleWorkspace && server && onConfigure && (
        <Button
          type="button"
          variant="secondary"
          disabled={busy}
          onClick={() => {
            setAuthentication('oauth');
            setRegistrationOptions({ dynamicRegistration: false });
            setExpanded(true);
          }}
        >
          Autorizar novamente com Google
        </Button>
      )}
      {expanded && (!isGitHub || !server || isGoogleWorkspace) && onConfigure && (
        <form id={formId} className="public-mcp-form" onSubmit={(event) => void submit(event)}>
          <label>
            URL do servidor
            <input
              type="url"
              value={endpoint}
              required
              readOnly={service.name === 'GitHub'}
              onChange={(event) => {
                setEndpoint(event.target.value);
                setClientId('');
                setRegistrationOptions(null);
                setOauthError('');
                setLinkError('');
              }}
            />
          </label>
          <label>
            Autenticação
            <select
              value={authentication}
              onChange={(event) => setAuthentication(event.target.value as 'oauth' | 'bearer')}
            >
              {service.authenticationMethods.map((method) => (
                <option key={method} value={method}>
                  {method === 'oauth' ? 'OAuth' : 'Token Bearer'}
                </option>
              ))}
            </select>
          </label>
          {authentication === 'bearer' && (
            <label>
              {service.name === 'GitHub' ? 'Token pessoal do GitHub (PAT)' : 'Token Bearer'}
              <input
                id={`${formId}-bearer-token`}
                name={`${formId}-bearer-token`}
                type="password"
                autoComplete="off"
                value={token}
                required
                onChange={(event) => setToken(event.target.value)}
              />
              <small>
                O token é salvo no Gerenciador de Credenciais e não volta a ser exibido.
              </small>
            </label>
          )}
          {authentication === 'oauth' && (
            <div className="public-mcp-oauth-options">
              {service.oauthRegistrationNote && (
                <div className="public-mcp-oauth-guide">
                  <p>{service.oauthRegistrationNote}</p>
                  {service.oauthSteps && (
                    <ol>
                      {service.oauthSteps.map((step) => (
                        <li key={step}>{step}</li>
                      ))}
                    </ol>
                  )}
                </div>
              )}
              {endpoint !== service.endpoint && (
                <label>
                  Identificador do cliente OAuth
                  <input
                    type="text"
                    name={`${formId}-oauth-client-id`}
                    id={`${formId}-oauth-client-id`}
                    autoComplete="off"
                    value={clientId}
                    onChange={(event) => setClientId(event.target.value)}
                  />
                  <small>Opcional quando o servidor oferece registro dinâmico.</small>
                </label>
              )}
              {endpoint === service.endpoint && oauthClientSettings?.clientId.trim() && (
                <p role="status">Identificador público configurado para este serviço.</p>
              )}
              {!isGoogleWorkspace && (
                <label>
                  Escopos OAuth (opcional)
                  <input
                    type="text"
                    name={`${formId}-oauth-scopes`}
                    id={`${formId}-oauth-scopes`}
                    value={scopes}
                    onChange={(event) => setScopes(event.target.value)}
                    placeholder="Separados por espaço"
                  />
                </label>
              )}
              <Button
                type="button"
                variant="secondary"
                disabled={checkingOAuth || busy || !endpoint.startsWith('https://')}
                onClick={() => void checkOAuthRegistration()}
              >
                {checkingOAuth ? 'Verificando…' : 'Verificar opções OAuth'}
              </Button>
              {registrationOptions?.dynamicRegistration && (
                <p role="status">
                  Registro dinâmico disponível. O identificador será registrado e salvo com a
                  autorização.
                </p>
              )}
              {oauthError && <p role="alert">{oauthError}</p>}
              {linkError && <p role="alert">{linkError}</p>}
            </div>
          )}
          <Button
            type="submit"
            disabled={
              !onConfigure ||
              busy ||
              (authentication === 'oauth' &&
                (!oauthClientSettings ||
                  (endpoint === service.endpoint
                    ? !publicOAuthReady
                    : !canConnectWithOAuth(registrationOptions, clientId, busy))))
            }
          >
            {authentication === 'oauth' ? 'Conectar e testar' : 'Salvar e testar conexão'}
          </Button>
        </form>
      )}
      {isGitHub &&
        server &&
        expanded &&
        authentication === 'bearer' &&
        onSaveGitHubToken &&
        onRemoveGitHubToken && (
          <GitHubTokenManager
            server={server}
            busy={busy}
            onSave={onSaveGitHubToken}
            onRemove={onRemoveGitHubToken}
          />
        )}
      {status === 'configured' && server && onTest && (
        <Button
          type="button"
          variant="secondary"
          disabled={busy}
          onClick={() => void onTest(server)}
        >
          Testar conexão
        </Button>
      )}
      {status === 'connected' && !githubAccount && (
        <p className="public-mcp-connected">Teste MCP concluído.</p>
      )}
    </article>
  );
}

function GitHubTokenManager({
  server,
  busy,
  onSave,
  onRemove,
}: {
  server: PublicMcpServer & { id: string };
  busy: boolean;
  onSave: NonNullable<SkillsCatalogViewProps['onSaveGitHubToken']>;
  onRemove: NonNullable<SkillsCatalogViewProps['onRemoveGitHubToken']>;
}) {
  const formId = useId();
  const [token, setToken] = useState('');
  const [notice, setNotice] = useState('');

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setNotice('');
    if (await onSave(server, token)) {
      setToken('');
      setNotice('Token salvo no Gerenciador de Credenciais. Ele não será exibido novamente.');
    }
  }

  async function remove() {
    setNotice('');
    if (await onRemove(server)) {
      setToken('');
      setNotice('Token removido do Gerenciador de Credenciais.');
    }
  }

  return (
    <form className="public-mcp-token-manager" onSubmit={(event) => void submit(event)}>
      <label htmlFor={`${formId}-token`}>Substituir token pessoal do GitHub</label>
      <input
        id={`${formId}-token`}
        name={`${formId}-token`}
        type="password"
        autoComplete="off"
        value={token}
        onChange={(event) => setToken(event.target.value)}
      />
      <div className="public-mcp-token-actions">
        <Button type="submit" disabled={busy || !token}>
          Salvar token
        </Button>
        <Button type="button" variant="secondary" disabled={busy} onClick={() => void remove()}>
          Remover token
        </Button>
      </div>
      <small>O valor salvo não pode ser consultado nem reaparece no OdeKrai.</small>
      {notice && <p role="status">{notice}</p>}
    </form>
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
  const [mcpServers, setMcpServers] = useState<Array<PublicMcpServer & { id: string }>>([]);
  const [mcpBusy, setMcpBusy] = useState(false);
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
      const [folders, apps, servers] = await Promise.all([
        invoke<Array<{ workspace: boolean }>>('get_allowed_folders').catch(() => null),
        invoke<Array<{ id: string }>>('get_allowed_apps').catch(() => null),
        invoke<Array<PublicMcpServer & { id: string }>>('mcp_list_servers').catch(() => null),
      ]);
      if (servers) setMcpServers(servers);
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

  async function configurePublicMcp(
    service: PublicMcpCatalogEntry,
    endpoint: string,
    authentication: 'oauth' | 'bearer',
    token: string,
    clientId: string,
    scopes: string,
  ): Promise<boolean> {
    if (!isTauri()) {
      setError('A configuração MCP só está disponível no aplicativo OdeKrai.');
      return false;
    }
    if (!endpoint.startsWith('https://')) {
      setError('O servidor MCP precisa usar uma URL HTTPS.');
      return false;
    }
    if (service.name === 'GitHub' && endpoint !== service.endpoint) {
      setError('O endereço oficial do servidor MCP do GitHub é fixo no catálogo.');
      return false;
    }
    if (authentication === 'bearer' && !token.trim()) {
      setError('Informe o token de autenticação.');
      return false;
    }
    setMcpBusy(true);
    setError('');
    let server: { id: string } | undefined;
    try {
      const existing = mcpServers.find(
        (item) => item.endpoint === endpoint && item.name === service.name,
      );
      server =
        existing ??
        (await invoke<{ id: string }>('mcp_add_http_server', {
          name: service.name,
          endpoint,
        }));
      if (authentication === 'bearer') {
        await invoke('mcp_set_http_token', { serverId: server.id, value: token });
        await invoke('mcp_set_enabled', { serverId: server.id, enabled: true });
      } else {
        await invoke('mcp_oauth_authorize', {
          serverId: server.id,
          clientId: clientId.trim() || null,
          scopes: scopes.trim() || null,
        });
      }
      await invoke('mcp_test_connection', { serverId: server.id, confirmed: true });
      await refreshMcpServers().catch(() => undefined);
      return true;
    } catch (cause) {
      const detail = String(cause);
      if (detail.includes('exige um identificador de cliente')) {
        setError(
          `Este servidor exige um identificador de cliente. Veja a documentação do serviço (${service.documentationUrl}) para criá-lo.`,
        );
      } else {
        setError(`Não foi possível configurar ou testar ${service.name}: ${detail}`);
      }
      await refreshMcpServers().catch(() => undefined);
      return false;
    } finally {
      setMcpBusy(false);
    }
  }

  const refreshMcpServers = useCallback(async () => {
    if (!isTauri()) return;
    const servers = await invoke<Array<PublicMcpServer & { id: string }>>('mcp_list_servers');
    setMcpServers(servers);
  }, []);

  async function testPublicMcp(server: PublicMcpServer & { id: string }): Promise<boolean> {
    setMcpBusy(true);
    setError('');
    try {
      if (!server.enabled) {
        await invoke('mcp_set_enabled', { serverId: server.id, enabled: true });
      }
      await invoke('mcp_test_connection', { serverId: server.id, confirmed: true });
      await refreshMcpServers().catch(() => undefined);
      return true;
    } catch (cause) {
      setError(`Não foi possível testar a conexão MCP: ${String(cause)}`);
      await refreshMcpServers().catch(() => undefined);
      return false;
    } finally {
      setMcpBusy(false);
    }
  }

  async function saveGitHubToken(
    server: PublicMcpServer & { id: string },
    token: string,
  ): Promise<boolean> {
    setMcpBusy(true);
    setError('');
    try {
      await invoke('mcp_set_http_token', { serverId: server.id, value: token });
      await refreshMcpServers().catch(() => undefined);
      return true;
    } catch (cause) {
      setError(`Não foi possível salvar o token do GitHub: ${String(cause)}`);
      return false;
    } finally {
      setMcpBusy(false);
    }
  }

  async function removeGitHubToken(server: PublicMcpServer & { id: string }): Promise<boolean> {
    setMcpBusy(true);
    setError('');
    try {
      await invoke('mcp_remove_http_token', { serverId: server.id });
      await refreshMcpServers().catch(() => undefined);
      return true;
    } catch (cause) {
      setError(`Não foi possível remover o token do GitHub: ${String(cause)}`);
      return false;
    } finally {
      setMcpBusy(false);
    }
  }

  async function connectGitHubOAuth(
    server: (PublicMcpServer & { id: string }) | undefined,
    clientId: string,
    scopes: string,
  ): Promise<boolean> {
    const service = publicMcpCatalog.find((item) => item.name === 'GitHub');
    if (!service) return false;
    if (!server) {
      return configurePublicMcp(service, service.endpoint, 'oauth', '', clientId, scopes);
    }
    if (server.endpoint !== service.endpoint) {
      throw new Error('O endereço salvo não corresponde ao servidor oficial do GitHub.');
    }
    setMcpBusy(true);
    setError('');
    try {
      await invoke('mcp_oauth_authorize', {
        serverId: server.id,
        clientId: clientId.trim() || null,
        scopes,
      });
      await invoke('mcp_test_connection', { serverId: server.id, confirmed: true });
      await refreshMcpServers().catch(() => undefined);
      return true;
    } catch (cause) {
      setError(`Não foi possível conectar ao GitHub: ${String(cause)}`);
      await refreshMcpServers().catch(() => undefined);
      throw cause;
    } finally {
      setMcpBusy(false);
    }
  }

  async function disconnectGitHubOAuth(
    server: PublicMcpServer & { id: string },
  ): Promise<string | null> {
    if (!isTauri()) throw new Error('A desconexão OAuth só está disponível no aplicativo OdeKrai.');
    setMcpBusy(true);
    try {
      const warning = await invoke<string | null>('mcp_oauth_disconnect_github', {
        serverId: server.id,
      });
      await refreshMcpServers().catch(() => undefined);
      return warning;
    } finally {
      setMcpBusy(false);
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
      mcpServers={mcpServers}
      mcpBusy={mcpBusy}
      onConfigurePublicMcp={configurePublicMcp}
      onSaveGitHubToken={saveGitHubToken}
      onRemoveGitHubToken={removeGitHubToken}
      onConnectGitHubOAuth={connectGitHubOAuth}
      onDisconnectGitHubOAuth={disconnectGitHubOAuth}
      onTestPublicMcp={testPublicMcp}
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

  async function saveGoogleCredentials(): Promise<boolean> {
    setBusy(true);
    try {
      const nextStatus = await invoke<GoogleOAuthStatus>('google_oauth_connect', {
        clientId,
        clientSecret: clientSecret || null,
        scopes: scopes.trim().split(/\s+/),
      });
      onStatusChange(nextStatus);
      return true;
    } catch (cause) {
      onError(String(cause));
      return false;
    } finally {
      setBusy(false);
    }
  }

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
            void submitSecretForm(event, saveGoogleCredentials, () => setClientSecret(''));
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
            name="google-client-secret"
            type="password"
            value={clientSecret}
            onChange={(event) => setClientSecret(event.target.value)}
            autoComplete="off"
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
