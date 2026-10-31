import { describe, expect, it, vi } from 'vitest';
import { AI_PROVIDERS } from './providers';
import { defaultSettings } from './settings';
import {
  filterProviderDirectory,
  loadSavedCredentialProviderIds,
  removeProviderCredential,
  removeSavedCredentialProviderId,
  setPrimaryProviderSettings,
  moveFallbackProvider,
  setFallbackProviderIds,
  setProviderEnabled,
  toggleProviderSettings,
  loadProviderDirectoryTab,
  saveProviderDirectoryTab,
  confirmProviderCredentialRemoval,
  providerCardHasManagement,
  isProviderConnected,
} from './provider-directory';

describe('diretório de provedores', () => {
  const connected = new Set(['groq', 'openai']);

  it('consulta credenciais em sequência e preserva as que foram encontradas', async () => {
    const calls: string[] = [];
    let activeCalls = 0;
    let maxActiveCalls = 0;
    const result = await loadSavedCredentialProviderIds(
      ['openai', 'groq', 'gemini'],
      async (id) => {
        calls.push(id);
        activeCalls += 1;
        maxActiveCalls = Math.max(maxActiveCalls, activeCalls);
        await Promise.resolve();
        activeCalls -= 1;
        if (id === 'groq') throw new Error('Credential Manager indisponível');
        return id === 'openai';
      },
    );

    expect(calls).toEqual(['openai', 'groq', 'gemini']);
    expect(maxActiveCalls).toBe(1);
    expect(result.credentialIds).toEqual(new Set(['openai']));
    expect(result.failedIds).toEqual(new Set(['groq']));
  });

  it('filtra conectados, gratuitos e todos por nome', () => {
    expect(
      filterProviderDirectory(AI_PROVIDERS, 'connected', '', connected).map((p) => p.id),
    ).toEqual(['openai', 'groq']);
    expect(filterProviderDirectory(AI_PROVIDERS, 'free', '', connected).map((p) => p.id)).toEqual([
      'gemini',
      'mistral',
      'groq',
      'openrouter',
    ]);
    expect(
      filterProviderDirectory(AI_PROVIDERS, 'all', 'cere', connected).map((p) => p.id),
    ).toEqual(['cerebras']);
    expect(filterProviderDirectory(AI_PROVIDERS, 'connected', '', new Set())).toEqual([]);
    expect(filterProviderDirectory(AI_PROVIDERS, 'all', '', connected)).toHaveLength(
      AI_PROVIDERS.length,
    );
  });

  it('só classifica credencial como conectada depois de teste aprovado', () => {
    expect(isProviderConnected('openai', true, undefined, false)).toBe(false);
    expect(isProviderConnected('openai', true, 'failed', false)).toBe(false);
    expect(isProviderConnected('openai', true, 'partial', false)).toBe(false);
    expect(isProviderConnected('openai', true, 'passed', false)).toBe(true);
    expect(isProviderConnected('openai', false, 'passed', false)).toBe(false);
  });

  it('classifica serviço local online independentemente da credencial opcional', () => {
    expect(isProviderConnected('local', false, undefined, true)).toBe(true);
    expect(isProviderConnected('local', false, undefined, false)).toBe(false);
  });

  it('não anuncia Cerebras como plano gratuito por causa de crédito promocional', () => {
    expect(AI_PROVIDERS.find((provider) => provider.id === 'cerebras')?.freePlan).toBeUndefined();
    expect(
      AI_PROVIDERS.filter((provider) => provider.freePlan?.status === 'verified').map(
        (provider) => provider.id,
      ),
    ).toEqual(['gemini', 'mistral', 'groq', 'openrouter']);
  });

  it('só mostra gerenciamento nos cartões conectados da aba Conectados', () => {
    expect(providerCardHasManagement('connected', true)).toBe(true);
    expect(providerCardHasManagement('free', true)).toBe(false);
    expect(providerCardHasManagement('all', true)).toBe(false);
    expect(providerCardHasManagement('connected', false)).toBe(false);
  });

  it('persiste e valida a aba selecionada', () => {
    const values = new Map<string, string>();
    const storage = {
      getItem: (key: string) => values.get(key) ?? null,
      setItem: (key: string, value: string) => values.set(key, value),
    };
    expect(loadProviderDirectoryTab(storage)).toBe('all');
    saveProviderDirectoryTab('connected', storage);
    expect(loadProviderDirectoryTab(storage)).toBe('connected');
    values.set('odekrai.provider-directory.tab', 'invalid');
    expect(loadProviderDirectoryTab(storage)).toBe('all');
  });

  it('ativa e desativa o provedor sem alterar os demais campos', () => {
    expect(setProviderEnabled({ model: 'm1', enabled: true }, false)).toEqual({
      model: 'm1',
      enabled: false,
    });
  });

  it('mantém a ordem dos provedores alternativos e exclui o principal', () => {
    const settings = {
      ...defaultSettings,
      activeProviderId: 'openai',
      fallbackProviderIds: ['anthropic', 'groq'],
    };
    expect(moveFallbackProvider(settings, 'groq', -1).fallbackProviderIds).toEqual([
      'groq',
      'anthropic',
    ]);
    expect(
      setFallbackProviderIds(settings, ['openai', 'groq', 'groq', 'invalid']).fallbackProviderIds,
    ).toEqual(['groq']);
    const disabledFallback = {
      ...settings,
      providers: {
        ...settings.providers,
        groq: { ...defaultSettings.providers.groq, enabled: false },
      },
    };
    expect(setFallbackProviderIds(disabledFallback, ['groq']).fallbackProviderIds).toEqual([]);
    expect(setPrimaryProviderSettings(settings, 'anthropic').fallbackProviderIds).toEqual(['groq']);
  });

  it('define principal e limpa a seleção quando esse provedor é desativado', () => {
    const primary = setPrimaryProviderSettings(defaultSettings, 'groq');
    expect(primary.activeProviderId).toBe('groq');
    expect(primary.providers.groq.enabled).toBe(true);
    const disabled = toggleProviderSettings(primary, 'groq', false);
    expect(disabled.activeProviderId).toBe('');
    expect(disabled.providers.groq.enabled).toBe(false);
    expect(disabled.fallbackProviderIds).not.toContain('groq');
  });

  it('remove o provedor dos IDs com credenciais salvas após remoção', () => {
    expect([...removeSavedCredentialProviderId(connected, 'groq')]).toEqual(['openai']);
  });

  it('só atualiza a lista depois que a remoção da credencial confirma sucesso', async () => {
    const remove = vi.fn().mockResolvedValue(undefined);
    await expect(removeProviderCredential('groq', remove, connected)).resolves.toEqual(
      new Set(['openai']),
    );
    expect(remove).toHaveBeenCalledWith('groq');
    const failed = vi.fn().mockRejectedValue(new Error('falhou'));
    await expect(removeProviderCredential('groq', failed, connected)).rejects.toThrow('falhou');
  });

  it('só remove credencial após confirmação explícita', async () => {
    const remove = vi.fn().mockResolvedValue(undefined);
    await expect(
      confirmProviderCredentialRemoval('OpenAI', async () => false, remove),
    ).resolves.toBe(false);
    expect(remove).not.toHaveBeenCalled();
    await expect(
      confirmProviderCredentialRemoval('OpenAI', async () => true, remove),
    ).resolves.toBe(true);
    expect(remove).toHaveBeenCalledOnce();
  });
});
