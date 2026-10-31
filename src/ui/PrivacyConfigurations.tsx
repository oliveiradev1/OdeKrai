import { useCallback, useEffect, useState, type FormEvent } from 'react';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { desktopDir, documentDir, downloadDir, pictureDir } from '@tauri-apps/api/path';
import { open } from '@tauri-apps/plugin-dialog';
import type { AppSettings } from '../core/settings';
import { normalizeTrustedLinkDomain } from '../core/markdown';
import { Button, SelectInput } from './DesignSystem';
import { Icon } from './Icon';
import { DiscordWebhooksSettings } from './DiscordWebhooksSettings';
import { useConfirmation } from './ConfirmationProvider';

type Folder = { path: string; access: 'read_only' | 'read_write'; workspace: boolean };
type AllowedApp = { id: string; name: string; path: string };
type McpServer = {
  id: string;
  name: string;
  command: string;
  args: string[];
  envNames: string[];
  enabled: boolean;
  status: string;
  error: string | null;
};
type McpState = { label: string; tools?: unknown; error?: string };

export function PrivacyConfigurationSummary({ onOpen }: { onOpen?: () => void }) {
  return (
    <section className="settings-section allowed-folders-section" aria-labelledby="privacy-summary">
      <div className="section-heading-row">
        <div>
          <h2 id="privacy-summary">Acessos e integrações</h2>
          <p>
            Pastas, aplicativos, links externos, destinos do Discord e servidores locais são
            consultados e alterados em Privacidade e permissões.
          </p>
        </div>
        {onOpen && (
          <Button type="button" variant="secondary" onClick={onOpen}>
            Abrir Privacidade e permissões
          </Button>
        )}
      </div>
    </section>
  );
}

export function PrivacyConfigurations({
  settings,
  update,
}: {
  settings: AppSettings;
  update: (settings: AppSettings) => void;
}) {
  const requestConfirmation = useConfirmation();
  const [folders, setFolders] = useState<Folder[]>([]);
  const [apps, setApps] = useState<AllowedApp[]>([]);
  const [access, setAccess] = useState<Folder['access']>('read_only');
  const [servers, setServers] = useState<McpServer[]>([]);
  const [serverStates, setServerStates] = useState<Record<string, McpState>>({});
  const [serverName, setServerName] = useState('');
  const [serverCommand, setServerCommand] = useState('');
  const [serverArgs, setServerArgs] = useState('');
  const [serverEnvNames, setServerEnvNames] = useState('');
  const [serverSecrets, setServerSecrets] = useState<Record<string, string>>({});
  const [serverConfirmed, setServerConfirmed] = useState(false);
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);

  const refresh = useCallback(async () => {
    if (!isTauri()) return;
    try {
      const [nextFolders, nextApps, nextServers] = await Promise.all([
        invoke<Folder[]>('get_allowed_folders'),
        invoke<AllowedApp[]>('get_allowed_apps'),
        invoke<McpServer[]>('mcp_list_servers'),
      ]);
      setFolders(nextFolders);
      setApps(nextApps);
      setServers(nextServers);
      setError('');
    } catch (cause) {
      setError(String(cause));
    }
  }, []);

  async function confirmAction(request: Parameters<typeof requestConfirmation>[0]) {
    try {
      return await requestConfirmation(request);
    } catch {
      setError('Não foi possível mostrar a confirmação. A ação não foi realizada.');
      return false;
    }
  }

  useEffect(() => {
    void refresh();
    const refreshOnFocus = () => void refresh();
    window.addEventListener('focus', refreshOnFocus);
    return () => window.removeEventListener('focus', refreshOnFocus);
  }, [refresh]);

  useEffect(() => {
    if (!isTauri()) return;
    let cancelled = false;
    void (async () => {
      for (const value of settings.trustedLinkDomains) {
        const domain = normalizeTrustedLinkDomain(value);
        if (domain) await invoke('trust_external_domain', { domain }).catch(() => undefined);
      }
      const domains = await invoke<string[]>('get_trusted_external_domains');
      if (!cancelled) update({ ...settings, trustedLinkDomains: domains });
    })().catch((cause: unknown) => {
      if (!cancelled) setError(`Não foi possível carregar os links lembrados: ${String(cause)}`);
    });
    return () => {
      cancelled = true;
    };
  }, []);

  async function addFolder(suggestion: string) {
    if (!isTauri()) return;
    const knownFolders: Record<string, () => Promise<string>> = {
      Documentos: documentDir,
      Downloads: downloadDir,
      'Área de Trabalho': desktopDir,
      Imagens: pictureDir,
    };
    const resolveDefaultPath = knownFolders[suggestion];
    if (!resolveDefaultPath) return;
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: `Escolha uma pasta para ${suggestion}`,
        defaultPath: await resolveDefaultPath(),
      });
      if (typeof selected !== 'string') return;
      setBusy(true);
      await invoke('add_allowed_folder', { path: selected, access });
      await refresh();
    } catch (cause) {
      setError(String(cause));
    } finally {
      setBusy(false);
    }
  }

  async function removeFolder(folder: Folder) {
    setBusy(true);
    try {
      await invoke('remove_allowed_folder', { path: folder.path });
      await refresh();
    } catch (cause) {
      setError(String(cause));
    } finally {
      setBusy(false);
    }
  }

  async function addApp() {
    if (!isTauri()) return;
    try {
      const selected = await open({
        multiple: false,
        directory: false,
        title: 'Escolha um aplicativo permitido',
        filters: [{ name: 'Aplicativos Windows', extensions: ['exe'] }],
      });
      if (typeof selected !== 'string') return;
      const suggestedName =
        selected
          .split(/[\\/]/)
          .at(-1)
          ?.replace(/\.exe$/i, '') ?? '';
      const name = window.prompt('Nome do aplicativo permitido', suggestedName)?.trim();
      if (!name) return;
      setBusy(true);
      await invoke('register_allowed_app', { name, path: selected });
      await refresh();
    } catch (cause) {
      setError(String(cause));
    } finally {
      setBusy(false);
    }
  }

  async function removeApp(app: AllowedApp) {
    setBusy(true);
    try {
      await invoke('remove_allowed_app', { appId: app.id });
      await refresh();
    } catch (cause) {
      setError(String(cause));
    } finally {
      setBusy(false);
    }
  }

  async function addServer() {
    if (!serverConfirmed) {
      setError('Confirme que este programa será executado com os privilégios da sua conta.');
      return;
    }
    setBusy(true);
    try {
      const args = serverArgs.split('\n').filter((line) => line.length > 0);
      const envNames = serverEnvNames
        .split('\n')
        .map((line) => line.trim())
        .filter(Boolean);
      const server = await invoke<McpServer>('mcp_add_server', {
        name: serverName,
        command: serverCommand,
        args,
        envNames,
      });
      try {
        for (const key of envNames) {
          const value = serverSecrets[key];
          if (value) await invoke('mcp_set_secret', { serverId: server.id, key, value });
        }
      } catch (cause) {
        await invoke('mcp_remove_server', { serverId: server.id }).catch(() => undefined);
        throw cause;
      }
      setServerSecrets({});
      setServerName('');
      setServerCommand('');
      setServerArgs('');
      setServerEnvNames('');
      setServerConfirmed(false);
      await refresh();
    } catch (cause) {
      setError(String(cause));
    } finally {
      setServerSecrets({});
      setBusy(false);
    }
  }

  async function testServer(server: McpServer) {
    const command = [server.command, ...server.args]
      .map((part) => `"${part.replaceAll('"', '\\"')}"`)
      .join(' ');
    if (
      !(await confirmAction({
        title: 'Executar programa local?',
        message: `Este programa será executado no seu computador com os privilégios da sua conta.\n\nComando completo:\n${command}\n\nDeseja continuar?`,
        destructive: true,
      }))
    )
      return;
    setServerStates((states) => ({ ...states, [server.id]: { label: 'Conectando…' } }));
    try {
      const tools = await invoke<unknown>('mcp_test_connection', {
        serverId: server.id,
        confirmed: true,
      });
      setServerStates((states) => ({ ...states, [server.id]: { label: 'Conectado', tools } }));
    } catch (cause) {
      setServerStates((states) => ({
        ...states,
        [server.id]: { label: 'Erro', error: String(cause) },
      }));
    }
  }

  async function saveServerSecret(server: McpServer, key: string) {
    const value = serverSecrets[`${server.id}:${key}`];
    if (!value) return;
    setBusy(true);
    try {
      await invoke('mcp_set_secret', { serverId: server.id, key, value });
      setServerStates((states) => ({ ...states, [server.id]: { label: 'Parado' } }));
      setServerSecrets((secrets) => {
        const next = { ...secrets };
        delete next[`${server.id}:${key}`];
        return next;
      });
      await refresh();
      setError('');
    } catch (cause) {
      setError(String(cause));
    } finally {
      setBusy(false);
    }
  }

  async function setServerEnabled(server: McpServer, enabled: boolean) {
    setBusy(true);
    try {
      await invoke('mcp_set_enabled', { serverId: server.id, enabled });
      setServerStates((states) => ({ ...states, [server.id]: { label: 'Parado' } }));
      await refresh();
    } catch (cause) {
      setError(String(cause));
    } finally {
      setBusy(false);
    }
  }

  async function trustDomain(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const form = event.currentTarget;
    const input = form.elements.namedItem('trusted-link-domain');
    const draft = input instanceof HTMLInputElement ? input.value.trim() : '';
    const domain = normalizeTrustedLinkDomain(draft);
    if (!domain || settings.trustedLinkDomains.includes(domain)) return;
    try {
      if (isTauri()) await invoke('trust_external_domain', { domain });
      update({
        ...settings,
        trustedLinkDomains: [...settings.trustedLinkDomains, domain].slice(0, 100),
      });
      form.reset();
      setError('');
    } catch {
      setError('Não foi possível lembrar esse domínio.');
    }
  }

  async function forgetDomain(domain: string) {
    try {
      if (isTauri()) await invoke('forget_external_domain', { domain });
      update({
        ...settings,
        trustedLinkDomains: settings.trustedLinkDomains.filter((item) => item !== domain),
      });
      setError('');
    } catch {
      setError('Não foi possível remover esse domínio da lista.');
    }
  }

  return (
    <div className="settings-embedded-content privacy-configurations">
      <PrivacyConfigurationSummary />
      {error && (
        <p role="alert" className="error-banner">
          {error}
        </p>
      )}
      <section className="settings-panel settings-section" aria-labelledby="trusted-links-heading">
        <h3 id="trusted-links-heading">Links externos lembrados</h3>
        <p className="settings-description">
          O aplicativo pede confirmação na primeira abertura. Você pode remover domínios lembrados.
        </p>
        <form className="fallback-provider-add" onSubmit={(event) => void trustDomain(event)}>
          <label className="sr-only" htmlFor="trusted-link-domain">
            Domínio confiável
          </label>
          <input
            id="trusted-link-domain"
            name="trusted-link-domain"
            placeholder="exemplo.com"
            maxLength={253}
          />
          <Button type="submit" variant="secondary">
            Lembrar domínio
          </Button>
        </form>
        <ul className="trusted-link-domains">
          {settings.trustedLinkDomains.map((domain) => (
            <li key={domain}>
              <span>{domain}</span>
              <Button
                type="button"
                variant="secondary"
                aria-label={`Esquecer ${domain}`}
                onClick={() => void forgetDomain(domain)}
              >
                Esquecer
              </Button>
            </li>
          ))}
        </ul>
      </section>
      <DiscordWebhooksSettings onChanged={refresh} />
      <section
        className="settings-panel settings-section allowed-folders-section"
        aria-labelledby="mcp-heading"
      >
        <h3 id="mcp-heading">Servidores locais</h3>
        <p>
          Cadastre servidores locais para inspecionar ferramentas e formatos. Eles não ficam
          disponíveis ao assistente. O programa cadastrado será executado no seu computador com os
          privilégios da sua conta.
        </p>
        <form
          className="mcp-server-form"
          onSubmit={(event) => {
            event.preventDefault();
            void addServer();
          }}
        >
          <div className="mcp-settings-grid">
            <label>
              Nome
              <input
                value={serverName}
                onChange={(event) => setServerName(event.target.value)}
                maxLength={80}
              />
            </label>
            <label>
              Comando ou caminho do programa
              <input
                value={serverCommand}
                onChange={(event) => setServerCommand(event.target.value)}
              />
            </label>
            <label>
              Argumentos (um por linha)
              <textarea
                value={serverArgs}
                onChange={(event) => setServerArgs(event.target.value)}
                rows={3}
              />
            </label>
            <label>
              Nomes das variáveis de ambiente (um por linha)
              <textarea
                value={serverEnvNames}
                onChange={(event) => setServerEnvNames(event.target.value)}
                rows={3}
              />
            </label>
            {serverEnvNames
              .split('\n')
              .map((key) => key.trim())
              .filter(Boolean)
              .map((key) => (
                <label key={key}>
                  {key} (opcional; salvo no Gerenciador de Credenciais)
                  <input
                    type="password"
                    autoComplete="new-password"
                    value={serverSecrets[key] ?? ''}
                    onChange={(event) =>
                      setServerSecrets((secrets) => ({ ...secrets, [key]: event.target.value }))
                    }
                  />
                </label>
              ))}
          </div>
          <p>
            Comando que será executado:{' '}
            <code>
              {[serverCommand, ...serverArgs.split('\n').filter(Boolean)]
                .map((part) => `"${part.replaceAll('"', '\\"')}"`)
                .join(' ') || '—'}
            </code>
          </p>
          <label className="skill-switch">
            <input
              type="checkbox"
              checked={serverConfirmed}
              onChange={(event) => setServerConfirmed(event.target.checked)}
            />{' '}
            Confirmo que entendo que o programa cadastrado será executado com os privilégios da
            minha conta.
          </label>
          <Button
            variant="secondary"
            type="submit"
            disabled={busy || !isTauri() || !serverConfirmed}
          >
            Adicionar servidor local
          </Button>
        </form>
        <ul className="allowed-folder-list">
          {servers.length === 0 && <li>Parado · Nenhum servidor local cadastrado.</li>}
          {servers.map((server) => {
            const state = serverStates[server.id] ?? {
              label: server.status,
              error: server.error ?? undefined,
            };
            const label = !server.enabled
              ? 'Parado'
              : ['Erro', 'Conectado'].includes(server.status)
                ? server.status
                : state.label;
            return (
              <li key={server.id}>
                <div>
                  <strong>{server.name}</strong>
                  <small className="mcp-server-command">
                    Comando: <code>{[server.command, ...server.args].join(' ')}</code>
                  </small>
                  <small>
                    Estado: <span role="status">{label}</span> ·{' '}
                    {server.enabled ? 'Ativado' : 'Desativado'}
                  </small>
                  {(server.error ?? state.error) && (
                    <p role="alert">Detalhes: {server.error ?? state.error}</p>
                  )}
                  {server.envNames.map((key) => (
                    <form
                      className="mcp-env-field"
                      key={key}
                      onSubmit={(event) => {
                        event.preventDefault();
                        void saveServerSecret(server, key);
                      }}
                    >
                      <label htmlFor={`mcp-secret-${server.id}-${key}`}>
                        {key} · credencial (opcional)
                      </label>
                      <input
                        id={`mcp-secret-${server.id}-${key}`}
                        type="password"
                        autoComplete="new-password"
                        value={serverSecrets[`${server.id}:${key}`] ?? ''}
                        onChange={(event) =>
                          setServerSecrets((secrets) => ({
                            ...secrets,
                            [`${server.id}:${key}`]: event.target.value,
                          }))
                        }
                      />
                      <Button
                        variant="secondary"
                        type="submit"
                        disabled={busy || !serverSecrets[`${server.id}:${key}`]}
                      >
                        Salvar credencial
                      </Button>
                    </form>
                  ))}
                  {state.tools !== undefined && (
                    <pre aria-label="Ferramentas e formatos não confiáveis">
                      {String(JSON.stringify(state.tools, null, 2))}
                    </pre>
                  )}
                  <div className="skill-switch">
                    <label>
                      <input
                        type="checkbox"
                        checked={server.enabled}
                        onChange={(event) => void setServerEnabled(server, event.target.checked)}
                      />{' '}
                      Ativar integração
                    </label>
                    <Button
                      variant="secondary"
                      disabled={!server.enabled || busy}
                      onClick={() => void testServer(server)}
                    >
                      Testar conexão
                    </Button>
                    <Button
                      variant="ghost"
                      disabled={busy}
                      onClick={() => {
                        void (async () => {
                          if (
                            !(await confirmAction({
                              title: 'Remover servidor?',
                              message: `Remover ${server.name} e apagar suas credenciais do Gerenciador de Credenciais?`,
                              destructive: true,
                            }))
                          )
                            return;
                          await invoke('mcp_remove_server', { serverId: server.id })
                            .then(refresh)
                            .catch((cause) => setError(String(cause)));
                        })();
                      }}
                    >
                      Remover
                    </Button>
                  </div>
                </div>
              </li>
            );
          })}
        </ul>
      </section>
      <section
        className="settings-panel settings-section allowed-folders-section"
        aria-labelledby="allowed-apps-heading"
      >
        <div className="section-heading-row">
          <div>
            <h3 id="allowed-apps-heading">Aplicativos permitidos</h3>
            <p>O assistente só pode iniciar os aplicativos escolhidos, sem argumentos.</p>
          </div>
          <Button variant="secondary" disabled={busy || !isTauri()} onClick={() => void addApp()}>
            <Icon name="folder" /> Escolher aplicativo
          </Button>
        </div>
        <ul className="allowed-folder-list">
          {apps.map((app) => (
            <li key={app.id}>
              <div>
                <strong>{app.name}</strong>
                <small>
                  {app.path} · {app.id}
                </small>
              </div>
              <Button
                variant="ghost"
                disabled={busy}
                onClick={() => void removeApp(app)}
                aria-label={`Remover ${app.name}`}
              >
                <Icon name="delete" />
              </Button>
            </li>
          ))}
        </ul>
      </section>
      <section
        className="settings-panel settings-section allowed-folders-section"
        aria-labelledby="allowed-folders-heading"
      >
        <div className="section-heading-row">
          <div>
            <h3 id="allowed-folders-heading">Pastas permitidas</h3>
            <p>
              A pasta de trabalho mantém leitura e escrita. Outras pastas só entram na lista depois
              que você as escolhe.
            </p>
          </div>
          <label>
            Nível de acesso
            <SelectInput
              value={access}
              onChange={(event) => setAccess(event.target.value as Folder['access'])}
            >
              <option value="read_only">Somente leitura</option>
              <option value="read_write">Leitura e escrita</option>
            </SelectInput>
          </label>
        </div>
        <div className="folder-shortcuts">
          {['Documentos', 'Downloads', 'Área de Trabalho', 'Imagens'].map((name) => (
            <Button
              key={name}
              variant="secondary"
              disabled={busy || !isTauri()}
              onClick={() => void addFolder(name)}
            >
              <Icon name="folder" /> Adicionar {name}
            </Button>
          ))}
        </div>
        <ul className="allowed-folder-list">
          {folders.map((folder) => (
            <li key={folder.path}>
              <div>
                <strong>{folder.path}</strong>
                <small>
                  {folder.workspace
                    ? 'Pasta de trabalho · leitura e escrita'
                    : folder.access === 'read_only'
                      ? 'Somente leitura'
                      : 'Leitura e escrita'}
                </small>
              </div>
              {!folder.workspace && (
                <Button
                  variant="ghost"
                  disabled={busy}
                  onClick={() => void removeFolder(folder)}
                  aria-label={`Remover ${folder.path}`}
                >
                  <Icon name="delete" />
                </Button>
              )}
            </li>
          ))}
        </ul>
      </section>
    </div>
  );
}
