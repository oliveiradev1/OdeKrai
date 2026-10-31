import { useEffect, useState } from 'react';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { defaultSettings, type AppSettings } from '../../core/settings';
import { AI_PROVIDERS } from '../../core/providers';
import { moveFallbackProvider, setFallbackProviderIds } from '../../core/provider-directory';
import { PageHeader, Toggle } from '../DesignSystem';
import { CodeDelegationSettings } from '../CodeDelegationSettings';
import { localBrazilianPortugueseVoices, type LocalSpeechVoice } from '../../core/speech';
import { openUrl } from '@tauri-apps/plugin-opener';
import {
  MICROSOFT_SPEECH_SETTINGS_URL,
  openMicrosoftSupportLink,
} from '../../core/microsoft-support-link';
import type { PerformanceModeView } from '../usePerformanceMode';

type ProviderUsage = {
  providerId: string;
  todayInput: number;
  todayOutput: number;
  sevenDayInput: number;
  sevenDayOutput: number;
};

type AgentLimits = {
  maxTurns: number;
  maxDurationSeconds: number;
  maxTokens: number;
  maxExternalActions: number;
};

export function SettingsScreen({
  settings,
  update,
  onOpenPermissions,
  performanceStatus,
  embedded = false,
}: {
  settings: AppSettings;
  update: (next: AppSettings) => void;
  onOpenPermissions: () => void;
  performanceStatus: PerformanceModeView;
  embedded?: boolean;
}) {
  const [usage, setUsage] = useState<ProviderUsage[]>([]);
  const [fallbackToAdd, setFallbackToAdd] = useState('');
  const [agentLimits, setAgentLimits] = useState<AgentLimits>({
    maxTurns: 8,
    maxDurationSeconds: 120,
    maxTokens: 16_000,
    maxExternalActions: 2,
  });
  const [agentLimitsError, setAgentLimitsError] = useState('');
  const [speechVoices, setSpeechVoices] = useState<LocalSpeechVoice[]>([]);
  const [speechHelpError, setSpeechHelpError] = useState('');
  const totalTokensToday = usage.reduce(
    (total, item) => total + item.todayInput + item.todayOutput,
    0,
  );
  const usageStatus =
    settings.dailyTokenLimit === 0
      ? 'Limite desativado'
      : totalTokensToday > settings.dailyTokenLimit
        ? 'Limite ultrapassado'
        : 'Dentro do limite';
  useEffect(() => {
    if (!('speechSynthesis' in window)) return;
    const refreshVoices = () =>
      setSpeechVoices(localBrazilianPortugueseVoices(window.speechSynthesis.getVoices()));
    refreshVoices();
    window.speechSynthesis.addEventListener('voiceschanged', refreshVoices);
    return () => window.speechSynthesis.removeEventListener('voiceschanged', refreshVoices);
  }, []);

  useEffect(() => {
    if (!isTauri()) return;
    void invoke<ProviderUsage[]>('get_provider_usage')
      .then(setUsage)
      .catch(() => setUsage([]));
    void invoke<AgentLimits>('get_agent_limits')
      .then(setAgentLimits)
      .catch(() => setAgentLimitsError('Não foi possível carregar os limites do agente.'));
  }, []);

  function updateAgentLimit<K extends keyof AgentLimits>(key: K, value: AgentLimits[K]) {
    const bounds: Record<keyof AgentLimits, [number, number]> = {
      maxTurns: [1, 32],
      maxDurationSeconds: [10, 600],
      maxTokens: [128, 200_000],
      maxExternalActions: [0, 20],
    };
    if (!Number.isSafeInteger(value) || value < bounds[key][0] || value > bounds[key][1]) return;
    const next = { ...agentLimits, [key]: value };
    setAgentLimits(next);
    if (!isTauri()) return;
    void invoke('set_agent_limits', { limits: next }).then(
      () => setAgentLimitsError(''),
      () => setAgentLimitsError('Não foi possível salvar os limites do agente.'),
    );
  }

  return (
    <div className={embedded ? 'settings-embedded-content' : 'page subpage'}>
      {!embedded && (
        <PageHeader
          eyebrow="PREFERÊNCIAS"
          title="Configurações"
          subtitle="Ajuste como o OdeKrai se comporta neste dispositivo."
        />
      )}
      <section id="settings-language" className="settings-panel settings-section">
        <h2>Geral</h2>
        <div className="panel-heading">
          <div>
            <h3>Idioma da interface</h3>
            <p>O idioma disponível nesta versão é Português (Brasil).</p>
          </div>
        </div>
        <div className="preference-row">
          <div>
            <strong>Idioma</strong>
          </div>
          <select value="pt-BR" disabled aria-label="Idioma da interface">
            <option value="pt-BR">Português (Brasil)</option>
          </select>
        </div>
      </section>
      <section id="settings-updates" className="settings-panel settings-section">
        <h2>Atualizações</h2>
        <div className="preference-row">
          <div>
            <strong>Mostrar novidades de novas versões</strong>
            <span>Exibe as notas da versão uma vez quando uma versão nova for aberta.</span>
          </div>
          <Toggle
            checked={settings.showReleaseNotes}
            label="Mostrar novidades de novas versões"
            onClick={() => update({ ...settings, showReleaseNotes: !settings.showReleaseNotes })}
          />
        </div>
      </section>
      <section id="settings-window" className="settings-panel settings-section">
        <h2>Janela</h2>
        <div className="preference-row">
          <div>
            <strong>Ao fechar a janela</strong>
            <span>Escolha se o botão X minimiza para a bandeja ou encerra o aplicativo.</span>
          </div>
          <select
            aria-label="Ação ao fechar a janela"
            value={settings.closeBehavior}
            onChange={(event) =>
              update({
                ...settings,
                closeBehavior: event.target.value as AppSettings['closeBehavior'],
              })
            }
          >
            <option value="minimize_to_tray">Minimizar para a bandeja</option>
            <option value="exit">Encerrar</option>
          </select>
        </div>
      </section>
      <section id="settings-performance" className="settings-panel settings-section">
        <h2>Desempenho</h2>
        <div className="preference-row">
          <div>
            <strong>Modo Desempenho</strong>
            <span>
              Automático ativa com movimento reduzido do Windows, até 8 GiB de RAM ou no máximo 4
              núcleos lógicos de CPU. Desligado desativa esta regra; o movimento reduzido do Windows
              continua sendo respeitado.
            </span>
          </div>
          <select
            aria-label="Modo Desempenho"
            value={settings.performanceMode}
            onChange={(event) =>
              update({
                ...settings,
                performanceMode: event.target.value as AppSettings['performanceMode'],
              })
            }
          >
            <option value="automatic">Automático</option>
            <option value="enabled">Ligado</option>
            <option value="disabled">Desligado</option>
          </select>
        </div>
        <p className="settings-description" role="status" aria-live="polite">
          {performanceStatus.loading
            ? 'Verificando movimento reduzido e recursos do computador…'
            : performanceStatus.reason}
        </p>
      </section>
      <section id="settings-appearance" className="settings-panel settings-section">
        <h2>Aparência</h2>
        <div className="preference-row">
          <div>
            <strong>Reduzir animações</strong>
            <span>Respeita a preferência de movimento reduzido.</span>
          </div>
          <Toggle
            checked={settings.reduceMotion}
            label="Reduzir animações"
            onClick={() => update({ ...settings, reduceMotion: !settings.reduceMotion })}
          />
        </div>
        <div className="preference-row">
          <div>
            <strong>Reduzir mascote</strong>
            <span>
              Usa uma imagem estática no Início e nos estados vazios. A janela do pet continua
              seguindo Reduzir animações e a preferência do sistema.
            </span>
          </div>
          <Toggle
            checked={settings.reduceMascot}
            label="Reduzir mascote"
            onClick={() => update({ ...settings, reduceMascot: !settings.reduceMascot })}
          />
        </div>
        <div className="preference-row">
          <div>
            <strong>Usar meu mascote como avatar do OdeKrai</strong>
            <span>Substitui o ícone do aplicativo nas respostas.</span>
          </div>
          <Toggle
            checked={settings.useMascotAsAssistantAvatar}
            label="Usar meu mascote como avatar do OdeKrai"
            onClick={() =>
              update({
                ...settings,
                useMascotAsAssistantAvatar: !settings.useMascotAsAssistantAvatar,
              })
            }
          />
        </div>
      </section>
      <section
        id="settings-audio"
        className="settings-panel settings-section"
        aria-labelledby="microphone-settings-heading"
      >
        <h2 id="microphone-settings-heading">Áudio</h2>
        <div className="preference-row">
          <div>
            <strong>Usar microfone</strong>
            <span>
              Permite gravar áudio localmente no campo de mensagem. Nada é enviado nesta etapa.
            </span>
          </div>
          <Toggle
            checked={settings.microphoneEnabled}
            label="Usar microfone"
            onClick={() => update({ ...settings, microphoneEnabled: !settings.microphoneEnabled })}
          />
        </div>
        <div className="preference-row">
          <div>
            <strong>Ler respostas automaticamente</strong>
            <span>Usa somente uma voz local instalada em português do Brasil.</span>
          </div>
          <Toggle
            checked={settings.readRepliesAutomatically}
            label="Ler respostas automaticamente"
            onClick={() =>
              update({ ...settings, readRepliesAutomatically: !settings.readRepliesAutomatically })
            }
          />
        </div>
        <div className="preference-row speech-setting-row">
          <label htmlFor="speech-voice">
            <strong>Voz em português (Brasil)</strong>
          </label>
          <select
            id="speech-voice"
            aria-label="Voz em português (Brasil)"
            value={
              speechVoices.some((voice) => voice.voiceURI === settings.speechVoiceURI)
                ? settings.speechVoiceURI
                : (speechVoices[0]?.voiceURI ?? '')
            }
            disabled={speechVoices.length === 0}
            onChange={(event) => update({ ...settings, speechVoiceURI: event.target.value })}
          >
            {speechVoices.length === 0 ? (
              <option value="">Nenhuma voz local disponível</option>
            ) : (
              speechVoices.map((voice) => (
                <option value={voice.voiceURI} key={voice.voiceURI}>
                  {voice.name} ({voice.lang})
                </option>
              ))
            )}
          </select>
        </div>
        {speechVoices.length === 0 && (
          <p className="settings-description" role="status">
            Não há voz local em português do Brasil disponível. No Windows, abra Configurações →
            Hora e idioma → Idioma e região → opções do Português (Brasil) e instale o recurso
            “Conversão de texto em fala”. Depois, feche e abra o OdeKrai.{' '}
            <a
              href={MICROSOFT_SPEECH_SETTINGS_URL}
              onClick={(event) => {
                event.preventDefault();
                void openMicrosoftSupportLink(MICROSOFT_SPEECH_SETTINGS_URL, openUrl).then(
                  () => setSpeechHelpError(''),
                  () => setSpeechHelpError('Não foi possível abrir as instruções da Microsoft.'),
                );
              }}
            >
              Instruções da Microsoft
            </a>
            .
          </p>
        )}
        {speechHelpError && <p role="alert">{speechHelpError}</p>}
        <div className="preference-row">
          <label htmlFor="speech-rate">
            <strong>Velocidade</strong>
          </label>
          <input
            id="speech-rate"
            type="range"
            min="0.5"
            max="2"
            step="0.1"
            value={settings.speechRate}
            onChange={(event) => update({ ...settings, speechRate: Number(event.target.value) })}
          />
          <output htmlFor="speech-rate">{settings.speechRate.toFixed(1)}×</output>
        </div>
        <div className="preference-row">
          <label htmlFor="speech-volume">
            <strong>Volume da leitura</strong>
          </label>
          <input
            id="speech-volume"
            type="range"
            min="0"
            max="1"
            step="0.1"
            value={settings.speechVolume}
            onChange={(event) => update({ ...settings, speechVolume: Number(event.target.value) })}
          />
          <output htmlFor="speech-volume">{Math.round(settings.speechVolume * 100)}%</output>
        </div>
      </section>
      <section
        id="settings-agent"
        className="settings-panel settings-section"
        aria-labelledby="agent-settings-heading"
      >
        <h2 id="agent-settings-heading">Agente</h2>
        <p className="settings-description">
          Os limites são aplicados pelo Rust em cada execução e ficam salvos neste dispositivo.
        </p>
        {agentLimitsError && <p role="alert">{agentLimitsError}</p>}
        <div className="preference-row">
          <label htmlFor="agent-max-turns">
            <strong>Voltas máximas</strong>
          </label>
          <input
            id="agent-max-turns"
            type="number"
            min="1"
            max="32"
            value={agentLimits.maxTurns}
            onChange={(event) => updateAgentLimit('maxTurns', Number(event.target.value))}
          />
        </div>
        <div className="preference-row">
          <label htmlFor="agent-max-duration">
            <strong>Tempo máximo (segundos)</strong>
          </label>
          <input
            id="agent-max-duration"
            type="number"
            min="10"
            max="600"
            step="10"
            value={agentLimits.maxDurationSeconds}
            onChange={(event) => updateAgentLimit('maxDurationSeconds', Number(event.target.value))}
          />
        </div>
        <div className="preference-row">
          <label htmlFor="agent-max-tokens">
            <strong>Tokens máximos por execução</strong>
          </label>
          <input
            id="agent-max-tokens"
            type="number"
            min="128"
            max="200000"
            step="128"
            value={agentLimits.maxTokens}
            onChange={(event) => updateAgentLimit('maxTokens', Number(event.target.value))}
          />
        </div>
        <div className="preference-row">
          <label htmlFor="agent-max-external">
            <strong>Ações de comunicação externa</strong>
          </label>
          <input
            id="agent-max-external"
            type="number"
            min="0"
            max="20"
            value={agentLimits.maxExternalActions}
            onChange={(event) => updateAgentLimit('maxExternalActions', Number(event.target.value))}
          />
        </div>
        <div className="agent-mode-help">
          <h3>Modos de permissão</h3>
          <ul>
            <li>
              <strong>Sempre perguntar:</strong> solicita aprovação antes de ações que alteram algo.
            </li>
            <li>
              <strong>Aprovar por mim:</strong> permite alterações reversíveis na pasta de trabalho;
              pergunta para as demais ações permitidas.
            </li>
            <li>
              <strong>Controle total:</strong> permite mais ações sem perguntar, mantendo bloqueios
              fixos e a confirmação nativa quando exigida.
            </li>
          </ul>
          <button
            type="button"
            className="secondary-button small-button"
            onClick={onOpenPermissions}
          >
            Abrir Permissões
          </button>
        </div>
      </section>
      <CodeDelegationSettings />
      <section
        id="settings-fallbacks"
        className="settings-panel settings-section"
        aria-labelledby="provider-fallback-heading"
      >
        <h2 id="provider-fallback-heading">Provedores alternativos</h2>
        <p className="settings-description">
          Se o provedor principal estiver temporariamente indisponível, o OdeKrai tentará os
          próximos desta lista. Provedores sem credencial ou capacidade compatível serão ignorados.
        </p>
        <div className="preference-row">
          <label htmlFor="fallback-provider-select">
            <strong>Adicionar provedor</strong>
          </label>
          <div className="fallback-provider-add">
            <select
              id="fallback-provider-select"
              value={fallbackToAdd}
              onChange={(event) => setFallbackToAdd(event.target.value)}
            >
              <option value="">Selecione</option>
              {AI_PROVIDERS.filter(
                (provider) =>
                  provider.integration !== 'guide' &&
                  provider.id !== settings.activeProviderId &&
                  settings.providers[provider.id]?.enabled !== false &&
                  !settings.fallbackProviderIds.includes(provider.id),
              ).map((provider) => (
                <option key={provider.id} value={provider.id}>
                  {provider.name}
                </option>
              ))}
            </select>
            <button
              className="secondary-button small-button"
              type="button"
              disabled={!fallbackToAdd}
              onClick={() => {
                update(
                  setFallbackProviderIds(settings, [
                    ...settings.fallbackProviderIds,
                    fallbackToAdd,
                  ]),
                );
                setFallbackToAdd('');
              }}
            >
              Adicionar
            </button>
          </div>
        </div>
        {settings.fallbackProviderIds.some((id) => settings.providers[id]?.enabled !== false) ? (
          <ol className="fallback-provider-list" aria-label="Ordem dos provedores alternativos">
            {settings.fallbackProviderIds
              .filter((id) => settings.providers[id]?.enabled !== false)
              .map((providerId, index, activeFallbacks) => {
                const provider = AI_PROVIDERS.find((item) => item.id === providerId);
                if (!provider) return null;
                return (
                  <li key={providerId}>
                    <span>{provider.name}</span>
                    <div>
                      <button
                        type="button"
                        className="secondary-button small-button"
                        aria-label={`Mover ${provider.name} para cima`}
                        disabled={index === 0}
                        onClick={() => update(moveFallbackProvider(settings, providerId, -1))}
                      >
                        ↑
                      </button>
                      <button
                        type="button"
                        className="secondary-button small-button"
                        aria-label={`Mover ${provider.name} para baixo`}
                        disabled={index === activeFallbacks.length - 1}
                        onClick={() => update(moveFallbackProvider(settings, providerId, 1))}
                      >
                        ↓
                      </button>
                      <button
                        type="button"
                        className="secondary-button small-button"
                        onClick={() =>
                          update(
                            setFallbackProviderIds(
                              settings,
                              settings.fallbackProviderIds.filter((id) => id !== providerId),
                            ),
                          )
                        }
                      >
                        Remover
                      </button>
                    </div>
                  </li>
                );
              })}
          </ol>
        ) : (
          <p className="settings-description">Nenhum provedor alternativo configurado.</p>
        )}
      </section>
      <section
        id="settings-usage"
        className="settings-panel settings-section"
        aria-labelledby="usage-heading"
      >
        <h2 id="usage-heading">Uso de tokens</h2>
        <p role="status" aria-live="polite">
          Estado: {usageStatus}
        </p>
        <div className="preference-row">
          <div>
            <strong>Limite diário total</strong>
            <span>
              Se uma resposta ultrapassar o limite, ela será descartada e as próximas serão
              bloqueadas. Use 0 para não definir limite.
            </span>
          </div>
          <input
            type="number"
            min="0"
            max="100000000"
            step="1000"
            aria-label="Limite diário de tokens"
            value={settings.dailyTokenLimit}
            onChange={(event) => {
              const value = Number(event.target.value);
              if (Number.isSafeInteger(value) && value >= 0 && value <= 100_000_000)
                update({ ...settings, dailyTokenLimit: value });
            }}
          />
        </div>
        <ul className="provider-usage-list" aria-label="Uso de tokens por provedor">
          {AI_PROVIDERS.filter((provider) =>
            usage.some((item) => item.providerId === provider.id),
          ).map((provider) => {
            const item = usage.find((entry) => entry.providerId === provider.id)!;
            return (
              <li key={provider.id}>
                <strong>{provider.name}</strong>
                <span>
                  Hoje: {(item.todayInput + item.todayOutput).toLocaleString('pt-BR')} tokens
                </span>
                <span>
                  Últimos 7 dias:{' '}
                  {(item.sevenDayInput + item.sevenDayOutput).toLocaleString('pt-BR')} tokens
                </span>
              </li>
            );
          })}
          {usage.length === 0 && (
            <li className="settings-description">Ainda não há uso registrado.</li>
          )}
        </ul>
      </section>
      <section id="settings-preferences-reset" className="settings-panel settings-section">
        <h2>Preferências deste dispositivo</h2>
        <p className="settings-description">
          Restaurar os valores iniciais não remove o perfil, as conversas nem as credenciais.
        </p>
        <button className="secondary-button small-button" onClick={() => update(defaultSettings)}>
          Restaurar preferências
        </button>
      </section>
      <div className="provider-footnote">
        <p>A restauração volta ao padrão Encerrar e não remove perfil, histórico ou credenciais.</p>
      </div>
    </div>
  );
}
