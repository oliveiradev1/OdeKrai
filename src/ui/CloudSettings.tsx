import { useEffect, useState, type FormEvent } from 'react';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { CLOUD_LINK_CATEGORIES, openCloudLink } from '../core/cloud-links';
import { submitSecretForm } from './secret-form';

type ExternalData<T> = {
  sourceLabel: string;
  untrusted: boolean;
  data: T;
};

type ShardVm = {
  id: string;
  name: string;
  status: string;
  ip: string;
  ram: number;
  vcpu: number;
  distro: string;
  description: string;
};

type ShardVmDetails = {
  vm: ShardVm;
  node: string;
  storage: number;
  template: string;
};

type ShardVmMetrics = {
  ram: number[];
  vcpu: number[];
  storage: number;
  networkReceive: number[];
  networkSend: number[];
};

type ShardDistro = { distro: string; version: string; value: string };
type HostingerStatus = 'Não conectado' | 'Conectado' | 'Erro';
type HostingerResult = { title: string; data: unknown };

export function hostingerStatusAfterRequest(
  current: HostingerStatus,
  connectionTest: boolean,
  succeeded: boolean,
): HostingerStatus {
  if (!succeeded) return 'Erro';
  return connectionTest ? 'Conectado' : current;
}

export function parseBudgetCents(input: string): number | null {
  const normalized = input.trim().replace(',', '.');
  if (!/^\d{1,7}(?:\.\d{1,2})?$/.test(normalized)) return null;
  const [reais, cents = ''] = normalized.split('.');
  const value = Number(reais) * 100 + Number(cents.padEnd(2, '0'));
  return Number.isSafeInteger(value) && value >= 100 && value <= 100_000_000 ? value : null;
}

function formatBudget(cents: number | null): string {
  if (cents === null) return '';
  return (cents / 100).toFixed(2).replace('.', ',');
}

export function CloudSettings({
  initialTokenSaved = false,
  initialConnected = false,
  initialBudgetCents = null,
  initialHostingerTokenSaved = false,
  initialHostingerStatus = 'Não conectado',
  initialHostingerVms = [],
}: {
  initialTokenSaved?: boolean;
  initialConnected?: boolean;
  initialBudgetCents?: number | null;
  initialHostingerTokenSaved?: boolean;
  initialHostingerStatus?: HostingerStatus;
  initialHostingerVms?: unknown[];
} = {}) {
  const [token, setToken] = useState('');
  const [tokenSaved, setTokenSaved] = useState(initialTokenSaved);
  const [credentialLoaded, setCredentialLoaded] = useState(() => !isTauri());
  const [connected, setConnected] = useState(initialConnected);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [vms, setVms] = useState<ShardVm[]>([]);
  const [selectedVmId, setSelectedVmId] = useState<string | null>(null);
  const [details, setDetails] = useState<ShardVmDetails | null>(null);
  const [metrics, setMetrics] = useState<ShardVmMetrics | null>(null);
  const [distros, setDistros] = useState<ShardDistro[] | null>(null);
  const [hostingerToken, setHostingerToken] = useState('');
  const [hostingerTokenSaved, setHostingerTokenSaved] = useState(initialHostingerTokenSaved);
  const [hostingerCredentialLoaded, setHostingerCredentialLoaded] = useState(() => !isTauri());
  const [hostingerStatus, setHostingerStatus] = useState<HostingerStatus>(initialHostingerStatus);
  const [hostingerBusy, setHostingerBusy] = useState(false);
  const [hostingerError, setHostingerError] = useState<string | null>(null);
  const [hostingerVms, setHostingerVms] = useState<unknown[]>(initialHostingerVms);
  const [hostingerResult, setHostingerResult] = useState<HostingerResult | null>(null);
  const [budgetCents, setBudgetCents] = useState<number | null>(initialBudgetCents);
  const [budgetInput, setBudgetInput] = useState(formatBudget(initialBudgetCents));

  function openCloudProvider(url: string) {
    void openCloudLink(url, (validatedUrl) =>
      invoke<void>('open_cloud_link', { url: validatedUrl }),
    ).catch((reason: unknown) => {
      setError(reason instanceof Error ? reason.message : safeUiError(reason));
    });
  }

  useEffect(() => {
    if (!isTauri()) {
      setCredentialLoaded(true);
      return;
    }
    let active = true;
    void invoke<boolean>('has_shardcloud_token')
      .then((saved) => {
        if (active) setTokenSaved(saved);
      })
      .catch(() => {
        if (active) setError('Não foi possível consultar o Gerenciador de Credenciais do Windows.');
      })
      .finally(() => {
        if (active) setCredentialLoaded(true);
      });
    void invoke<number | null>('get_shardcloud_monthly_budget')
      .then((cents) => {
        if (active) {
          setBudgetCents(cents);
          setBudgetInput(formatBudget(cents));
        }
      })
      .catch(() => {
        if (active) setError('Não foi possível carregar o teto mensal da ShardCloud.');
      });
    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    if (!isTauri()) {
      setHostingerCredentialLoaded(true);
      return;
    }
    let active = true;
    void invoke<boolean>('has_hostinger_token')
      .then((saved) => {
        if (active) setHostingerTokenSaved(saved);
      })
      .catch(() => {
        if (active) {
          setHostingerError(
            'Não foi possível consultar o Gerenciador de Credenciais do Windows para a Hostinger.',
          );
          setHostingerStatus('Erro');
        }
      })
      .finally(() => {
        if (active) setHostingerCredentialLoaded(true);
      });
    return () => {
      active = false;
    };
  }, []);

  async function saveBudget(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!isTauri()) return;
    const cents = budgetInput.trim() ? parseBudgetCents(budgetInput) : null;
    if (budgetInput.trim() && cents === null) {
      setError('Informe um teto entre R$ 1,00 e R$ 1.000.000,00, com até duas casas decimais.');
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await invoke('set_shardcloud_monthly_budget', { cents });
      setBudgetCents(cents);
      setBudgetInput(formatBudget(cents));
    } catch (reason) {
      setError(safeUiError(reason));
    } finally {
      setBusy(false);
    }
  }

  async function saveToken(): Promise<boolean> {
    if (!isTauri()) return false;
    setBusy(true);
    setError(null);
    try {
      await invoke('save_shardcloud_token', { token });
      setTokenSaved(true);
      setConnected(false);
      setVms([]);
      setDetails(null);
      setMetrics(null);
      setDistros(null);
      return true;
    } catch (reason) {
      setError(safeUiError(reason));
      return false;
    } finally {
      setBusy(false);
    }
  }

  async function testConnection() {
    if (!isTauri() || !tokenSaved) return;
    setBusy(true);
    setError(null);
    try {
      const result = await invoke<ExternalData<ShardVm[]>>('shardcloud_test_connection');
      setVms(result.data);
      setConnected(true);
      setDetails(null);
      setMetrics(null);
    } catch (reason) {
      setConnected(false);
      setError(safeUiError(reason));
    } finally {
      setBusy(false);
    }
  }

  async function refreshVms() {
    if (!isTauri() || !tokenSaved) return;
    setBusy(true);
    setError(null);
    try {
      const result = await invoke<ExternalData<ShardVm[]>>('shardcloud_list_vms');
      setVms(result.data);
    } catch (reason) {
      setError(safeUiError(reason));
    } finally {
      setBusy(false);
    }
  }

  async function loadDetails(vmId: string) {
    if (!isTauri()) return;
    setSelectedVmId(vmId);
    setBusy(true);
    setError(null);
    try {
      const result = await invoke<ExternalData<ShardVmDetails>>('shardcloud_get_vm_details', {
        vmId,
      });
      setDetails(result.data);
      setMetrics(null);
    } catch (reason) {
      setError(safeUiError(reason));
    } finally {
      setBusy(false);
    }
  }

  async function loadMetrics(vmId: string) {
    if (!isTauri()) return;
    setSelectedVmId(vmId);
    setBusy(true);
    setError(null);
    try {
      const result = await invoke<ExternalData<ShardVmMetrics>>('shardcloud_get_vm_metrics', {
        vmId,
      });
      setMetrics(result.data);
      setDetails(null);
    } catch (reason) {
      setError(safeUiError(reason));
    } finally {
      setBusy(false);
    }
  }

  async function loadDistros() {
    if (!isTauri()) return;
    setBusy(true);
    setError(null);
    try {
      const result = await invoke<ExternalData<ShardDistro[]>>('shardcloud_list_distros');
      setDistros(result.data);
    } catch (reason) {
      setError(safeUiError(reason));
    } finally {
      setBusy(false);
    }
  }

  async function removeToken() {
    if (!isTauri()) return;
    setBusy(true);
    setError(null);
    try {
      await invoke('remove_shardcloud_token');
      setTokenSaved(false);
      setConnected(false);
      setVms([]);
      setDetails(null);
      setMetrics(null);
      setDistros(null);
      setSelectedVmId(null);
    } catch (reason) {
      setError(safeUiError(reason));
    } finally {
      setBusy(false);
    }
  }

  async function saveHostingerToken(): Promise<boolean> {
    if (!isTauri()) return false;
    setHostingerBusy(true);
    setHostingerError(null);
    try {
      await invoke('save_hostinger_token', { token: hostingerToken });
      setHostingerTokenSaved(true);
      setHostingerStatus('Não conectado');
      setHostingerVms([]);
      setHostingerResult(null);
      return true;
    } catch (reason) {
      setHostingerStatus('Erro');
      setHostingerError(safeHostingerError(reason));
      return false;
    } finally {
      setHostingerBusy(false);
    }
  }

  async function removeHostingerToken() {
    if (!isTauri()) return;
    setHostingerBusy(true);
    setHostingerError(null);
    try {
      await invoke('remove_hostinger_token');
      setHostingerTokenSaved(false);
      setHostingerStatus('Não conectado');
      setHostingerVms([]);
      setHostingerResult(null);
    } catch (reason) {
      setHostingerStatus('Erro');
      setHostingerError(safeHostingerError(reason));
    } finally {
      setHostingerBusy(false);
    }
  }

  async function runHostingerRead(
    command: string,
    title: string,
    args: Record<string, unknown> = {},
    connectionTest = false,
  ) {
    if (!isTauri() || !hostingerTokenSaved) return;
    setHostingerBusy(true);
    setHostingerError(null);
    try {
      const result = await invoke<ExternalData<unknown>>(command, args);
      const data = result.data;
      setHostingerStatus(hostingerStatusAfterRequest(hostingerStatus, connectionTest, true));
      if (connectionTest) {
        setHostingerVms(Array.isArray(data) ? data : []);
      }
      if (command === 'hostinger_list_vms') {
        setHostingerVms(Array.isArray(data) ? data : []);
      }
      setHostingerResult({ title, data });
    } catch (reason) {
      setHostingerStatus(hostingerStatusAfterRequest(hostingerStatus, connectionTest, false));
      setHostingerError(safeHostingerError(reason));
    } finally {
      setHostingerBusy(false);
    }
  }

  return (
    <div className="settings-panel settings-section cloud-settings">
      <h2 id="cloud-settings-heading">Nuvem</h2>
      <p>
        A integração permite consultar suas VMs. O agente também pode iniciar ou parar uma VM da
        lista consultada e executar de 1 a 10 comandos nela. O conteúdo integral dos comandos é
        enviado à API ShardCloud somente depois da aprovação na janela nativa, em qualquer modo.
      </p>
      <p className="cloud-settings-notice">
        Nenhuma consulta de rede ocorre ao salvar o token. Ele fica no Gerenciador de Credenciais do
        Windows e não é exibido novamente depois de salvo.
      </p>

      <section className="cloud-budget" aria-labelledby="cloud-budget-heading">
        <h3 id="cloud-budget-heading">Teto mensal para VMs</h3>
        <form onSubmit={(event) => void saveBudget(event)}>
          <label htmlFor="shardcloud-monthly-budget">Valor em reais por mês</label>
          <div className="cloud-actions">
            <input
              id="shardcloud-monthly-budget"
              type="text"
              inputMode="decimal"
              autoComplete="off"
              value={budgetInput}
              onChange={(event) => setBudgetInput(event.target.value)}
              placeholder="Ex.: 100,00"
              aria-describedby="shardcloud-budget-note"
            />
            <button type="submit" disabled={busy}>
              Salvar teto
            </button>
            {budgetCents !== null && (
              <button
                type="button"
                disabled={busy}
                onClick={() => {
                  setBudgetInput('');
                  void invoke('set_shardcloud_monthly_budget', { cents: null })
                    .then(() => setBudgetCents(null))
                    .catch((reason) => setError(safeUiError(reason)));
                }}
              >
                Remover teto
              </button>
            )}
          </div>
        </form>
        <p id="shardcloud-budget-note" className="cloud-settings-notice">
          {budgetCents === null
            ? 'Sem teto salvo, a criação de VMs deve permanecer bloqueada.'
            : `Teto salvo: R$ ${formatBudget(budgetCents)} por mês.`}
        </p>
        <p className="cloud-settings-notice">
          O preço real por VM não foi conferido. Este valor é uma referência local; a ShardCloud não
          recebe nem aplica esse teto. Custos podem variar conforme o plano e os recursos.
        </p>
      </section>

      <section className="cloud-shardcloud" aria-labelledby="cloud-shardcloud-heading">
        <div className="cloud-shardcloud-title">
          <h3 id="cloud-shardcloud-heading">ShardCloud</h3>
          {connected ? (
            <span className="cloud-connected-label">Conectado</span>
          ) : tokenSaved ? (
            <span className="cloud-saved-label">Token salvo</span>
          ) : (
            <span className="cloud-planned-label">Configuração necessária</span>
          )}
        </div>

        {!credentialLoaded ? (
          <p role="status">Consultando credencial salva…</p>
        ) : tokenSaved ? (
          <div className="cloud-token-saved">
            <p>Token salvo no Gerenciador de Credenciais do Windows.</p>
            <div className="cloud-actions">
              <button type="button" disabled={busy} onClick={() => void testConnection()}>
                {busy ? 'Consultando…' : 'Testar conexão'}
              </button>
              <button type="button" disabled={busy} onClick={() => void removeToken()}>
                Remover
              </button>
            </div>
          </div>
        ) : (
          <form
            className="cloud-token-entry"
            onSubmit={(event) => {
              void submitSecretForm(event, saveToken, () => setToken(''));
            }}
          >
            <label htmlFor="shardcloud-token">Token da ShardCloud</label>
            <input
              id="shardcloud-token"
              name="shardcloud-token"
              type="password"
              autoComplete="off"
              value={token}
              onChange={(event) => setToken(event.target.value)}
              placeholder="Cole o token da ShardCloud"
              spellCheck={false}
            />
            <button type="submit" disabled={busy || !token.trim()}>
              {busy ? 'Salvando…' : 'Salvar token'}
            </button>
          </form>
        )}

        {error && (
          <p className="cloud-error" role="alert">
            {error}
          </p>
        )}

        {connected && (
          <div className="cloud-readonly-data">
            <div className="cloud-readonly-heading">
              <h4>Máquinas virtuais</h4>
              <button type="button" disabled={busy} onClick={() => void refreshVms()}>
                Atualizar VMs
              </button>
            </div>
            <p className="cloud-untrusted-label">Dados externos da ShardCloud — não confiáveis</p>
            {vms.length === 0 ? (
              <p>Nenhuma VM foi retornada para esta conta.</p>
            ) : (
              <ul className="cloud-vm-list">
                {vms.map((vm) => (
                  <li key={vm.id}>
                    <div>
                      <strong>{vm.name || 'VM sem nome'}</strong>
                      <span>{vm.status || 'Estado não informado'}</span>
                      <small>
                        {vm.distro || 'Distribuição não informada'} · {vm.ram} MB RAM · {vm.vcpu}{' '}
                        vCPU
                      </small>
                      {vm.description && <small>{vm.description}</small>}
                    </div>
                    <div className="cloud-actions">
                      <button
                        type="button"
                        disabled={busy}
                        aria-pressed={selectedVmId === vm.id && Boolean(details)}
                        onClick={() => void loadDetails(vm.id)}
                      >
                        Detalhes
                      </button>
                      <button
                        type="button"
                        disabled={busy}
                        aria-pressed={selectedVmId === vm.id && Boolean(metrics)}
                        onClick={() => void loadMetrics(vm.id)}
                      >
                        Métricas
                      </button>
                    </div>
                  </li>
                ))}
              </ul>
            )}

            {details && (
              <section className="cloud-vm-result" aria-label="Detalhes da VM">
                <h4>Detalhes de {details.vm.name || 'VM'}</h4>
                <p className="cloud-untrusted-label">
                  Dados externos da ShardCloud — não confiáveis
                </p>
                <dl>
                  <dt>Endereço IP</dt>
                  <dd>{details.vm.ip || 'Não informado'}</dd>
                  <dt>Nó</dt>
                  <dd>{details.node || 'Não informado'}</dd>
                  <dt>Armazenamento</dt>
                  <dd>{details.storage} GB</dd>
                  <dt>Modelo</dt>
                  <dd>{details.template || 'Não informado'}</dd>
                </dl>
              </section>
            )}

            {metrics && (
              <section className="cloud-vm-result" aria-label="Métricas da VM">
                <h4>Métricas de {selectedVmId}</h4>
                <p className="cloud-untrusted-label">
                  Métricas externas da ShardCloud — não confiáveis
                </p>
                <dl>
                  <dt>Amostras de RAM</dt>
                  <dd>{metrics.ram.length}</dd>
                  <dt>Amostras de vCPU</dt>
                  <dd>{metrics.vcpu.length}</dd>
                  <dt>Amostras de rede recebida</dt>
                  <dd>{metrics.networkReceive.length}</dd>
                  <dt>Amostras de rede enviada</dt>
                  <dd>{metrics.networkSend.length}</dd>
                  <dt>Armazenamento</dt>
                  <dd>{metrics.storage} GB</dd>
                </dl>
              </section>
            )}

            <section className="cloud-distros">
              <div className="cloud-readonly-heading">
                <h4>Distribuições disponíveis</h4>
                <button type="button" disabled={busy} onClick={() => void loadDistros()}>
                  Consultar distribuições
                </button>
              </div>
              {distros && (
                <>
                  <p className="cloud-untrusted-label">
                    Dados externos da ShardCloud — não confiáveis
                  </p>
                  {distros.length ? (
                    <ul>
                      {distros.map((distro, index) => (
                        <li key={`${distro.value}-${index}`}>
                          {distro.distro} {distro.version} <small>({distro.value})</small>
                        </li>
                      ))}
                    </ul>
                  ) : (
                    <p>Nenhuma distribuição foi retornada.</p>
                  )}
                </>
              )}
            </section>
          </div>
        )}
      </section>

      <section className="cloud-hostinger" aria-labelledby="cloud-hostinger-heading">
        <div className="cloud-shardcloud-title">
          <h3 id="cloud-hostinger-heading">Hostinger</h3>
          <span
            className={
              hostingerStatus === 'Conectado'
                ? 'cloud-connected-label'
                : hostingerStatus === 'Erro'
                  ? 'cloud-error-label'
                  : 'cloud-saved-label'
            }
            role="status"
          >
            {hostingerStatus}
          </span>
        </div>
        <p>
          A integração consulta VPS, detalhes, data centers, templates de sistema, ações recentes e
          backups. As ações de iniciar, parar ou reiniciar pelo agente sempre pedem sua aprovação. O
          token fica no Gerenciador de Credenciais do Windows e não é exibido novamente.
        </p>
        {!hostingerCredentialLoaded ? (
          <p role="status">Consultando credencial salva…</p>
        ) : hostingerTokenSaved ? (
          <div className="cloud-token-saved">
            <p>Token salvo</p>
            <div className="cloud-actions">
              <button
                type="button"
                disabled={hostingerBusy}
                onClick={() =>
                  void runHostingerRead('hostinger_test_connection', 'Máquinas virtuais', {}, true)
                }
              >
                {hostingerBusy ? 'Consultando…' : 'Testar conexão'}
              </button>
              <button
                type="button"
                disabled={hostingerBusy}
                onClick={() => void removeHostingerToken()}
              >
                Remover
              </button>
            </div>
          </div>
        ) : (
          <form
            className="cloud-token-entry"
            onSubmit={(event) => {
              void submitSecretForm(event, saveHostingerToken, () => setHostingerToken(''));
            }}
          >
            <label htmlFor="hostinger-token">Token da Hostinger</label>
            <input
              id="hostinger-token"
              name="hostinger-token"
              type="password"
              autoComplete="off"
              value={hostingerToken}
              onChange={(event) => setHostingerToken(event.target.value)}
              placeholder="Cole o token da Hostinger"
              spellCheck={false}
            />
            <button type="submit" disabled={hostingerBusy || !hostingerToken.trim()}>
              {hostingerBusy ? 'Salvando…' : 'Salvar token'}
            </button>
          </form>
        )}
        {hostingerError && (
          <p className="cloud-error" role="alert">
            {hostingerError}
          </p>
        )}
        {hostingerTokenSaved && hostingerStatus === 'Conectado' && (
          <div className="cloud-readonly-data">
            <div className="cloud-readonly-heading">
              <h4>Máquinas virtuais</h4>
              <button
                type="button"
                disabled={hostingerBusy}
                onClick={() => void runHostingerRead('hostinger_list_vms', 'Máquinas virtuais')}
              >
                Atualizar máquinas
              </button>
            </div>
            <p className="cloud-untrusted-label">Dados externos da Hostinger — não confiáveis</p>
            {hostingerVms.length === 0 ? (
              <p>Nenhuma máquina virtual foi retornada.</p>
            ) : (
              <ul className="cloud-vm-list">
                {hostingerVms.map((value, index) => {
                  const vm = asRecord(value);
                  const vmId = typeof vm?.id === 'number' ? vm.id : null;
                  return (
                    <li key={vmId ?? index}>
                      <div>
                        <strong>{readExternalText(vm?.hostname) || 'VPS sem nome'}</strong>
                        {vmId !== null && <small>Identificador: {vmId}</small>}
                        <span>{readExternalText(vm?.state) || 'Estado não informado'}</span>
                        <small>{readExternalText(vm?.plan) || 'Plano não informado'}</small>
                      </div>
                      {vmId !== null && (
                        <div className="cloud-actions">
                          <button
                            type="button"
                            disabled={hostingerBusy}
                            onClick={() =>
                              void runHostingerRead('hostinger_get_vm_details', 'Detalhes da VPS', {
                                vmId,
                              })
                            }
                          >
                            Detalhes
                          </button>
                          <button
                            type="button"
                            disabled={hostingerBusy}
                            onClick={() =>
                              void runHostingerRead(
                                'hostinger_list_recent_actions',
                                'Ações recentes',
                                { vmId },
                              )
                            }
                          >
                            Ações recentes
                          </button>
                          <button
                            type="button"
                            disabled={hostingerBusy}
                            onClick={() =>
                              void runHostingerRead('hostinger_list_backups', 'Backups', { vmId })
                            }
                          >
                            Backups
                          </button>
                        </div>
                      )}
                    </li>
                  );
                })}
              </ul>
            )}
            <div className="cloud-actions">
              <button
                type="button"
                disabled={hostingerBusy}
                onClick={() => void runHostingerRead('hostinger_list_data_centers', 'Data centers')}
              >
                Data centers
              </button>
              <button
                type="button"
                disabled={hostingerBusy}
                onClick={() =>
                  void runHostingerRead('hostinger_list_templates', 'Templates de sistema')
                }
              >
                Templates de sistema
              </button>
            </div>
            {hostingerResult && (
              <section className="cloud-vm-result" aria-label={hostingerResult.title}>
                <h4>{hostingerResult.title}</h4>
                <p className="cloud-untrusted-label">
                  Dados externos da Hostinger — não confiáveis
                </p>
                <pre>{JSON.stringify(hostingerResult.data, null, 2)}</pre>
              </section>
            )}
          </div>
        )}
      </section>

      <p className="cloud-settings-notice">
        Iniciar, parar ou executar comandos em uma VM exige aprovação em qualquer modo. A prévia
        mostra o nome e o ID conferidos da VM; para comandos, revise o lote completo antes de
        aprovar. Comandos podem ler ou alterar dados da VM e usar a rede dela; parar pode
        interromper serviços em execução.
      </p>
      {CLOUD_LINK_CATEGORIES.map((category) => (
        <CloudProviderList
          key={category.title}
          title={category.title}
          services={category.services}
          onOpen={openCloudProvider}
        />
      ))}
    </div>
  );
}

function safeUiError(reason: unknown): string {
  if (typeof reason !== 'string') return 'Não foi possível concluir a solicitação à ShardCloud.';
  return reason.slice(0, 400);
}

function safeHostingerError(reason: unknown): string {
  return typeof reason === 'string'
    ? reason.slice(0, 400)
    : 'Não foi possível concluir a solicitação à Hostinger.';
}

function asRecord(value: unknown): Record<string, unknown> | null {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : null;
}

function readExternalText(value: unknown): string {
  return typeof value === 'string' ? value : '';
}

function CloudProviderList({
  title,
  services,
  onOpen,
}: {
  title: string;
  services: readonly { name: string; url: string }[];
  onOpen: (url: string) => void;
}) {
  return (
    <section className="cloud-provider-list" aria-label={title}>
      <h3>{title}</h3>
      <ul>
        {services.map((service) => (
          <li key={service.name}>
            <a
              href={service.url}
              onClick={(event) => {
                event.preventDefault();
                onOpen(service.url);
              }}
            >
              {service.name}
            </a>
            <span className="cloud-planned-label">Planejado</span>
          </li>
        ))}
      </ul>
    </section>
  );
}
