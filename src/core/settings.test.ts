import { afterEach, describe, expect, it, vi } from 'vitest';
import { defaultSettings, loadSettings, loadSettingsWithWarnings, saveSettings } from './settings';
import { shouldAutomaticallyCheckForUpdates } from './update-check';

describe('preferências persistidas', () => {
  afterEach(() => vi.unstubAllGlobals());

  it('ativa a verificação automática por padrão e migra preferências antigas', () => {
    expect(defaultSettings.checkForUpdatesAutomatically).toBe(true);
    vi.stubGlobal('localStorage', {
      getItem: () =>
        JSON.stringify({ ...defaultSettings, checkForUpdatesAutomatically: undefined }),
      setItem: () => undefined,
    });
    expect(loadSettings().checkForUpdatesAutomatically).toBe(true);
  });

  it('respeita a opção desligada e só automatiza no aplicativo nativo', () => {
    expect(shouldAutomaticallyCheckForUpdates(false, true)).toBe(false);
    expect(shouldAutomaticallyCheckForUpdates(true, false)).toBe(false);
    expect(shouldAutomaticallyCheckForUpdates(true, true)).toBe(true);
  });

  it('migra preferências antigas preenchendo valores padrão do Companion', () => {
    const storage = new Map<string, string>();
    storage.set(
      'odekrai.settings.v1',
      JSON.stringify({
        language: 'pt-BR',
        reduceMotion: true,
        companion: { mascotType: 'legacy' },
        provider: { name: 'Local', baseUrl: 'http://localhost:11434/v1', model: 'local' },
      }),
    );
    vi.stubGlobal('localStorage', {
      getItem: (key: string) => storage.get(key) ?? null,
      setItem: (key: string, value: string) => storage.set(key, value),
    });
    const migrated = loadSettings();
    expect(migrated.reduceMascot).toBe(false);
    expect(migrated.companion).toMatchObject({
      enabled: false,
      position: 'bottom-right',
      sounds: { enabled: true },
    });
    expect(migrated.companion).not.toHaveProperty('mascotType');
    expect(migrated.providers.openai.authMode).toBe('bearer');
    expect(migrated.providers.openai.providerId).toBe('openai');
    expect(migrated.providers.openai.baseUrl).toBe('https://api.openai.com/v1');
    expect(migrated.providers.openai.model).toBe('local');
    saveSettings(migrated);
    expect(JSON.parse(storage.get('odekrai.settings.v2') ?? '{}')).not.toHaveProperty('provider');
  });

  it('preserva o resultado sanitizado do último teste do conector ao carregar preferências', () => {
    vi.stubGlobal('localStorage', {
      getItem: (key: string) =>
        key === 'odekrai.settings.v2'
          ? JSON.stringify({
              ...defaultSettings,
              activeProviderId: 'groq',
              providers: {
                groq: {
                  providerId: 'groq',
                  name: 'Groq',
                  baseUrl: 'https://api.groq.com/openai/v1',
                  model: 'mock-model',
                  authMode: 'bearer',
                  enabled: true,
                  lastTest: {
                    status: 'passed',
                    at: '2026-10-28T12:00:00.000Z',
                    message: 'Teste de conexão aprovado.',
                  },
                },
              },
            })
          : null,
      setItem: () => undefined,
    });

    expect(loadSettings().providers.groq.lastTest).toMatchObject({ status: 'passed' });
  });

  it('salva somente preferências e recupera a posição da ilha', () => {
    const storage = new Map<string, string>();
    vi.stubGlobal('localStorage', {
      getItem: (key: string) => storage.get(key) ?? null,
      setItem: (key: string, value: string) => storage.set(key, value),
    });
    const settings = loadSettings();
    settings.companion.coordinates = { x: 120, y: 240 };
    settings.companion.freeMode = true;
    settings.companion.desktopCoordinates = { x: 410, y: 80 };
    expect(saveSettings(settings)).toBe(true);
    expect(loadSettings().companion.coordinates).toEqual({ x: 120, y: 240 });
    expect(loadSettings().companion).toMatchObject({
      freeMode: true,
      desktopCoordinates: { x: 410, y: 80 },
    });
    expect(storage.get('odekrai.settings.v2')).not.toContain('apiKey');
    expect(storage.get('odekrai.settings.v2')).not.toContain('chave');
  });

  it('persiste a preferência independente para reduzir o mascote', () => {
    const storage = new Map<string, string>();
    vi.stubGlobal('localStorage', {
      getItem: (key: string) => storage.get(key) ?? null,
      setItem: (key: string, value: string) => storage.set(key, value),
    });
    const settings = loadSettings();
    settings.reduceMascot = true;
    expect(saveSettings(settings)).toBe(true);
    expect(loadSettings().reduceMascot).toBe(true);
  });

  it('usa novidades ativadas por padrão e migra a opção persistida', () => {
    expect(defaultSettings.showReleaseNotes).toBe(true);
    vi.stubGlobal('localStorage', {
      getItem: (key: string) =>
        key === 'odekrai.settings.v2'
          ? JSON.stringify({ ...defaultSettings, showReleaseNotes: false })
          : null,
      setItem: () => undefined,
    });
    expect(loadSettings().showReleaseNotes).toBe(false);
  });

  it('inicia Desempenho em Automático e persiste a escolha manual', () => {
    const storage = new Map<string, string>();
    vi.stubGlobal('localStorage', {
      getItem: (key: string) => storage.get(key) ?? null,
      setItem: (key: string, value: string) => storage.set(key, value),
    });
    const settings = loadSettings();
    expect(settings.performanceMode).toBe('automatic');
    settings.performanceMode = 'enabled';
    expect(saveSettings(settings)).toBe(true);
    expect(loadSettings().performanceMode).toBe('enabled');
  });

  it('persiste a substituição de efeito sonoro selecionada', () => {
    const storage = new Map<string, string>();
    vi.stubGlobal('localStorage', {
      getItem: (key: string) => storage.get(key) ?? null,
      setItem: (key: string, value: string) => storage.set(key, value),
    });
    const settings = loadSettings();
    settings.companion.sounds.replacements.success = '/sounds/error.ogg';

    expect(saveSettings(settings)).toBe(true);
    expect(loadSettings().companion.sounds.replacements.success).toBe('/sounds/error.ogg');
  });

  it('persiste o estado e as larguras dos painéis, limitando valores inválidos', () => {
    const storage = new Map<string, string>();
    vi.stubGlobal('localStorage', {
      getItem: (key: string) => storage.get(key) ?? null,
      setItem: (key: string, value: string) => storage.set(key, value),
    });
    const settings = loadSettings();
    settings.layout.sidebarCollapsed = true;
    settings.layout.sidebarWidth = 320;
    settings.layout.conversationSidebarCollapsed = true;
    settings.layout.conversationSidebarWidth = 280;
    settings.trustedLinkDomains = ['example.com'];
    expect(saveSettings(settings)).toBe(true);
    expect(loadSettings().layout).toEqual(settings.layout);
    expect(loadSettings().trustedLinkDomains).toEqual(['example.com']);

    storage.set(
      'odekrai.settings.v2',
      JSON.stringify({ ...settings, layout: { sidebarWidth: 900, conversationSidebarWidth: 20 } }),
    );
    expect(loadSettings().layout).toMatchObject({
      sidebarWidth: 340,
      conversationSidebarWidth: 170,
    });
  });

  it('usa Encerrar por padrão e restaura valores inválidos de fechamento', () => {
    const storage = new Map<string, string>();
    vi.stubGlobal('localStorage', {
      getItem: (key: string) => storage.get(key) ?? null,
      setItem: (key: string, value: string) => storage.set(key, value),
    });
    expect(loadSettings().closeBehavior).toBe('exit');
    storage.set(
      'odekrai.settings.v2',
      JSON.stringify({ ...loadSettings(), closeBehavior: 'other' }),
    );
    expect(loadSettings().closeBehavior).toBe('exit');
    storage.set(
      'odekrai.settings.v2',
      JSON.stringify({ ...loadSettings(), closeBehavior: 'minimize_to_tray' }),
    );
    expect(loadSettings().closeBehavior).toBe('minimize_to_tray');
  });

  it('mantém a preferência de usar o mascote como avatar do OdeKrai', () => {
    const storage = new Map<string, string>();
    vi.stubGlobal('localStorage', {
      getItem: (key: string) => storage.get(key) ?? null,
      setItem: (key: string, value: string) => storage.set(key, value),
    });
    const settings = loadSettings();
    expect(settings.useMascotAsAssistantAvatar).toBe(false);
    settings.useMascotAsAssistantAvatar = true;
    saveSettings(settings);
    expect(loadSettings().useMascotAsAssistantAvatar).toBe(true);
  });

  it('migra o ID pelo nome antigo e preserva um ID válido quando o nome muda', () => {
    let value = JSON.stringify({
      language: 'pt-BR',
      reduceMotion: false,
      provider: { name: 'Anthropic', baseUrl: 'https://api.anthropic.com', model: 'claude' },
    });
    vi.stubGlobal('localStorage', { getItem: () => value, setItem: () => undefined });
    expect(loadSettings().providers.anthropic.providerId).toBe('anthropic');
    expect(loadSettings().providers.anthropic.authMode).toBe('x-api-key');
    value = JSON.stringify({
      language: 'pt-BR',
      reduceMotion: false,
      provider: {
        providerId: 'anthropic',
        name: 'Nome alterado',
        baseUrl: 'https://api.anthropic.com',
        model: 'claude',
      },
    });
    expect(loadSettings().providers.anthropic.providerId).toBe('anthropic');
  });

  it('normaliza a ordem dos provedores alternativos e o limite diário de tokens', () => {
    const storage = new Map<string, string>();
    storage.set(
      'odekrai.settings.v2',
      JSON.stringify({
        ...loadSettings(),
        fallbackProviderIds: ['anthropic', 'anthropic', 'openai', 'missing'],
        dailyTokenLimit: -1,
      }),
    );
    vi.stubGlobal('localStorage', {
      getItem: (key: string) => storage.get(key) ?? null,
      setItem: (key: string, value: string) => storage.set(key, value),
    });
    expect(loadSettings().fallbackProviderIds).toEqual(['anthropic']);
    expect(loadSettings().dailyTokenLimit).toBe(100_000);
  });

  it('mantém endpoints e modelos independentes ao trocar de provedor', () => {
    const storage = new Map<string, string>();
    vi.stubGlobal('localStorage', {
      getItem: (key: string) => storage.get(key) ?? null,
      setItem: (key: string, value: string) => storage.set(key, value),
    });
    const settings = loadSettings();
    settings.providers.groq = {
      providerId: 'groq',
      name: 'Groq',
      baseUrl: 'https://api.groq.com/openai/v1',
      model: 'llama-3.3-70b-versatile',
      authMode: 'bearer',
    };
    settings.activeProviderId = 'groq';
    saveSettings(settings);
    const loaded = loadSettings();
    expect(loaded.activeProviderId).toBe('groq');
    expect(loaded.providers.groq.model).toBe('llama-3.3-70b-versatile');
    expect(loaded.providers.openai).toBeUndefined();
  });

  it('restaura endpoint salvo de outro provedor e informa o usuário', () => {
    const value = JSON.stringify({
      language: 'pt-BR',
      reduceMotion: false,
      activeProviderId: 'groq',
      providers: {
        groq: {
          providerId: 'groq',
          name: 'Groq',
          baseUrl: 'https://api.x.ai/v1/responses',
          model: 'modelo-customizado',
          authMode: 'bearer',
        },
      },
    });
    vi.stubGlobal('localStorage', { getItem: () => value, setItem: () => undefined });
    const result = loadSettingsWithWarnings();
    expect(result.settings.providers.groq.baseUrl).toBe('https://api.groq.com/openai/v1');
    expect(result.settings.providers.groq.model).toBe('modelo-customizado');
    expect(result.warnings).toHaveLength(1);
    expect(result.warnings[0]).toContain('Groq');
  });

  it('mantém a ausência de provedor principal após desativar todos', () => {
    const storage = new Map<string, string>();
    vi.stubGlobal('localStorage', {
      getItem: (key: string) => storage.get(key) ?? null,
      setItem: (key: string, value: string) => storage.set(key, value),
    });
    const settings = loadSettings();
    settings.activeProviderId = '';
    saveSettings(settings);
    expect(loadSettings().activeProviderId).toBe('');
  });

  it('persiste o estado ativado ou desativado de um provedor', () => {
    const storage = new Map<string, string>();
    vi.stubGlobal('localStorage', {
      getItem: (key: string) => storage.get(key) ?? null,
      setItem: (key: string, value: string) => storage.set(key, value),
    });
    const settings = loadSettings();
    settings.providers.groq = {
      providerId: 'groq',
      name: 'Groq',
      baseUrl: 'https://api.groq.com/openai/v1',
      model: 'test-model',
      authMode: 'bearer',
      enabled: false,
    };
    saveSettings(settings);
    expect(loadSettings().providers.groq.enabled).toBe(false);
  });

  it('mantém leitura automática desligada por padrão e limita as preferências de voz', () => {
    expect(defaultSettings.readRepliesAutomatically).toBe(false);
    const saved = {
      ...defaultSettings,
      readRepliesAutomatically: true,
      speechVoiceURI: 'voice-local-pt-br',
      speechRate: 8,
      speechVolume: -2,
    };
    vi.stubGlobal('localStorage', {
      getItem: (key: string) => (key === 'odekrai.settings.v2' ? JSON.stringify(saved) : null),
      setItem: () => undefined,
    });
    expect(loadSettings()).toMatchObject({
      readRepliesAutomatically: true,
      speechVoiceURI: 'voice-local-pt-br',
      speechRate: 2,
      speechVolume: 0,
    });
  });
});
