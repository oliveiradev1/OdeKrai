import { normalizeSoundPreferences } from './sounds';
import type { PerformanceMode } from './performance';
import {
  AI_PROVIDERS,
  providerById,
  providerCatalogEntry,
  type ProviderAuthMode,
} from './providers';

export type ProviderSettings = {
  providerId: string;
  name: string;
  baseUrl: string;
  model: string;
  authMode: ProviderAuthMode;
  region?: string;
  lastTest?: { status: 'passed' | 'failed' | 'partial'; at: string; message: string };
  enabled?: boolean;
};

export type AppSettings = {
  language: 'pt-BR' | 'en';
  reduceMotion: boolean;
  reduceMascot: boolean;
  useMascotAsAssistantAvatar: boolean;
  showReleaseNotes: boolean;
  checkForUpdatesAutomatically: boolean;
  performanceMode: PerformanceMode;
  microphoneEnabled: boolean;
  readRepliesAutomatically: boolean;
  speechVoiceURI: string;
  speechRate: number;
  speechVolume: number;
  closeBehavior: 'minimize_to_tray' | 'exit';
  activeProviderId: string;
  fallbackProviderIds: string[];
  dailyTokenLimit: number;
  trustedLinkDomains: string[];
  layout: {
    sidebarCollapsed: boolean;
    sidebarWidth: number;
    conversationSidebarCollapsed: boolean;
    conversationSidebarWidth: number;
  };
  providers: Record<string, ProviderSettings>;
  companion: {
    enabled: boolean;
    position: 'top-right' | 'bottom-right' | 'top-left' | 'bottom-left';
    size: number;
    alwaysOnTop: boolean;
    openOnHover: boolean;
    collapseSeconds: number;
    coordinates: { x: number; y: number } | null;
    freeMode: boolean;
    desktopCoordinates: { x: number; y: number } | null;
    sounds: {
      enabled: boolean;
      volume: number;
      replacements: Partial<Record<import('./sounds').SoundEvent, string>>;
    };
  };
};

const STORAGE_KEY = 'odekrai.settings.v2';
const LEGACY_STORAGE_KEY = 'odekrai.settings.v1';

export function defaultProviderSettings(providerId: string): ProviderSettings {
  const provider = providerById(providerId) ?? AI_PROVIDERS[0];
  return {
    providerId: provider.id,
    name: provider.name,
    baseUrl: provider.baseUrl ?? '',
    model: '',
    authMode: provider.authMode,
    enabled: true,
    region: provider.region ? 'us-east-1' : undefined,
  };
}

export function activeProviderSettings(settings: AppSettings): ProviderSettings {
  return (
    settings.providers[settings.activeProviderId] ??
    defaultProviderSettings(settings.activeProviderId)
  );
}

export const defaultSettings: AppSettings = {
  language: 'pt-BR',
  reduceMotion: false,
  reduceMascot: false,
  useMascotAsAssistantAvatar: false,
  showReleaseNotes: true,
  checkForUpdatesAutomatically: true,
  performanceMode: 'automatic',
  microphoneEnabled: false,
  readRepliesAutomatically: false,
  speechVoiceURI: '',
  speechRate: 1,
  speechVolume: 1,
  closeBehavior: 'exit',
  activeProviderId: 'openai',
  fallbackProviderIds: [],
  dailyTokenLimit: 100_000,
  trustedLinkDomains: [],
  layout: {
    sidebarCollapsed: false,
    sidebarWidth: 254,
    conversationSidebarCollapsed: false,
    conversationSidebarWidth: 230,
  },
  providers: {},
  companion: {
    enabled: false,
    position: 'bottom-right',
    size: 1,
    alwaysOnTop: true,
    openOnHover: false,
    collapseSeconds: 15,
    coordinates: null,
    freeMode: false,
    desktopCoordinates: null,
    sounds: { enabled: true, volume: 0.2, replacements: {} },
  },
};

export function loadSettings(): AppSettings {
  return loadSettingsWithWarnings().settings;
}

export function loadSettingsWithWarnings(): { settings: AppSettings; warnings: string[] } {
  const warnings: string[] = [];
  try {
    const raw = localStorage.getItem(STORAGE_KEY) ?? localStorage.getItem(LEGACY_STORAGE_KEY);
    if (!raw) return { settings: defaultSettings, warnings };
    const value: unknown = JSON.parse(raw);
    if (!isAppSettings(value)) return { settings: defaultSettings, warnings };
    const companion =
      value.companion && typeof value.companion === 'object'
        ? (value.companion as Partial<AppSettings['companion']>)
        : {};
    const companionValues = {
      ...(value.companion && typeof value.companion === 'object'
        ? value.companion
        : defaultSettings.companion),
    } as AppSettings['companion'] & { mascotType?: unknown };
    delete companionValues.mascotType;
    const position = ['top-right', 'bottom-right', 'top-left', 'bottom-left'].includes(
      String(companion.position),
    )
      ? companion.position!
      : defaultSettings.companion.position;
    const coordinates =
      companion.coordinates &&
      Number.isFinite(companion.coordinates.x) &&
      Number.isFinite(companion.coordinates.y) &&
      Math.abs(companion.coordinates.x) <= 100_000 &&
      Math.abs(companion.coordinates.y) <= 100_000
        ? { x: companion.coordinates.x, y: companion.coordinates.y }
        : null;
    const desktopCoordinates =
      companion.desktopCoordinates &&
      Number.isFinite(companion.desktopCoordinates.x) &&
      Number.isFinite(companion.desktopCoordinates.y) &&
      Math.abs(companion.desktopCoordinates.x) <= 100_000 &&
      Math.abs(companion.desktopCoordinates.y) <= 100_000
        ? { x: companion.desktopCoordinates.x, y: companion.desktopCoordinates.y }
        : null;
    const legacy = 'provider' in value ? (value.provider as Partial<ProviderSettings>) : undefined;
    const oldId =
      legacy && typeof legacy === 'object'
        ? typeof legacy.providerId === 'string' && providerById(legacy.providerId)
          ? legacy.providerId
          : (AI_PROVIDERS.find((provider) => provider.name === legacy.name)?.id ?? 'openai')
        : undefined;
    const rawProviders =
      'providers' in value && value.providers && typeof value.providers === 'object'
        ? (value.providers as Record<string, unknown>)
        : {};
    const providers: Record<string, ProviderSettings> = {};
    for (const provider of AI_PROVIDERS) {
      const candidate = rawProviders[provider.id];
      if (candidate && typeof candidate === 'object') {
        const saved = candidate as Partial<ProviderSettings>;
        const savedBaseUrl =
          typeof saved.baseUrl === 'string' ? saved.baseUrl : (provider.baseUrl ?? '');
        const hostRule = providerCatalogEntry(provider.id)?.hostRule;
        const baseUrl =
          hostRule && !providerEndpointHostAllowed(savedBaseUrl, hostRule)
            ? (warnings.push(
                `O endpoint salvo para ${provider.name} foi restaurado ao padrão por não pertencer aos domínios permitidos.`,
              ),
              provider.baseUrl ?? '')
            : savedBaseUrl;
        providers[provider.id] = {
          ...defaultProviderSettings(provider.id),
          baseUrl,
          model: typeof saved.model === 'string' ? saved.model : '',
          enabled: typeof saved.enabled === 'boolean' ? saved.enabled : true,
          region: typeof saved.region === 'string' ? saved.region : undefined,
          lastTest: validTest(saved.lastTest) ? saved.lastTest : undefined,
        };
      }
    }
    if (oldId && legacy && !providers[oldId]) {
      const defaultUrl = providerById(oldId)?.baseUrl ?? '';
      const candidateUrl = typeof legacy.baseUrl === 'string' ? legacy.baseUrl : defaultUrl;
      const hostRule = providerCatalogEntry(oldId)?.hostRule;
      const baseUrl =
        hostRule && !providerEndpointHostAllowed(candidateUrl, hostRule)
          ? (warnings.push(
              `O endpoint salvo para ${providerById(oldId)?.name ?? oldId} foi restaurado ao padrão por não pertencer aos domínios permitidos.`,
            ),
            defaultUrl)
          : candidateUrl;
      providers[oldId] = {
        ...defaultProviderSettings(oldId),
        baseUrl,
        model: typeof legacy.model === 'string' ? legacy.model : '',
        region: typeof legacy.region === 'string' ? legacy.region : undefined,
      };
    }
    const activeProviderId =
      'activeProviderId' in value &&
      typeof value.activeProviderId === 'string' &&
      (value.activeProviderId === '' || providerById(value.activeProviderId))
        ? value.activeProviderId
        : (oldId ?? 'openai');
    const fallbackProviderIds = Array.isArray(value.fallbackProviderIds)
      ? [
          ...new Set(
            value.fallbackProviderIds.filter(
              (id): id is string =>
                typeof id === 'string' && Boolean(providerById(id)) && id !== activeProviderId,
            ),
          ),
        ].slice(0, 15)
      : [];
    const dailyTokenLimit =
      typeof value.dailyTokenLimit === 'number' &&
      Number.isSafeInteger(value.dailyTokenLimit) &&
      value.dailyTokenLimit >= 0 &&
      value.dailyTokenLimit <= 100_000_000
        ? value.dailyTokenLimit
        : defaultSettings.dailyTokenLimit;
    const layoutValue =
      value.layout && typeof value.layout === 'object'
        ? (value.layout as Partial<AppSettings['layout']>)
        : {};
    const trustedLinkDomains = Array.isArray(value.trustedLinkDomains)
      ? [
          ...new Set(
            value.trustedLinkDomains.filter(
              (domain): domain is string =>
                typeof domain === 'string' && /^[a-z0-9.-]+$/.test(domain) && domain.length <= 253,
            ),
          ),
        ].slice(0, 100)
      : [];
    const migrated = { ...value } as Record<string, unknown>;
    delete migrated.provider;
    return {
      settings: {
        ...defaultSettings,
        ...migrated,
        reduceMascot: value.reduceMascot === true,
        useMascotAsAssistantAvatar:
          typeof value.useMascotAsAssistantAvatar === 'boolean'
            ? value.useMascotAsAssistantAvatar
            : false,
        showReleaseNotes:
          typeof value.showReleaseNotes === 'boolean'
            ? value.showReleaseNotes
            : defaultSettings.showReleaseNotes,
        checkForUpdatesAutomatically:
          typeof value.checkForUpdatesAutomatically === 'boolean'
            ? value.checkForUpdatesAutomatically
            : defaultSettings.checkForUpdatesAutomatically,
        performanceMode: ['automatic', 'enabled', 'disabled'].includes(value.performanceMode)
          ? (value.performanceMode as PerformanceMode)
          : defaultSettings.performanceMode,
        microphoneEnabled: value.microphoneEnabled === true,
        readRepliesAutomatically: value.readRepliesAutomatically === true,
        speechVoiceURI:
          typeof value.speechVoiceURI === 'string' && value.speechVoiceURI.length <= 512
            ? value.speechVoiceURI
            : '',
        speechRate:
          typeof value.speechRate === 'number' && Number.isFinite(value.speechRate)
            ? Math.max(0.5, Math.min(2, value.speechRate))
            : 1,
        speechVolume:
          typeof value.speechVolume === 'number' && Number.isFinite(value.speechVolume)
            ? Math.max(0, Math.min(1, value.speechVolume))
            : 1,
        closeBehavior: value.closeBehavior === 'minimize_to_tray' ? 'minimize_to_tray' : 'exit',
        activeProviderId,
        fallbackProviderIds,
        dailyTokenLimit,
        trustedLinkDomains,
        layout: {
          sidebarCollapsed:
            typeof layoutValue.sidebarCollapsed === 'boolean'
              ? layoutValue.sidebarCollapsed
              : defaultSettings.layout.sidebarCollapsed,
          sidebarWidth:
            typeof layoutValue.sidebarWidth === 'number' &&
            Number.isFinite(layoutValue.sidebarWidth)
              ? Math.max(190, Math.min(340, layoutValue.sidebarWidth))
              : defaultSettings.layout.sidebarWidth,
          conversationSidebarCollapsed:
            typeof layoutValue.conversationSidebarCollapsed === 'boolean'
              ? layoutValue.conversationSidebarCollapsed
              : defaultSettings.layout.conversationSidebarCollapsed,
          conversationSidebarWidth:
            typeof layoutValue.conversationSidebarWidth === 'number' &&
            Number.isFinite(layoutValue.conversationSidebarWidth)
              ? Math.max(170, Math.min(360, layoutValue.conversationSidebarWidth))
              : defaultSettings.layout.conversationSidebarWidth,
        },
        providers,
        companion: {
          ...defaultSettings.companion,
          ...companionValues,
          enabled: typeof companion.enabled === 'boolean' ? companion.enabled : false,
          position,
          size: Number.isFinite(companion.size)
            ? Math.max(0.75, Math.min(1.5, companion.size!))
            : 1,
          alwaysOnTop: typeof companion.alwaysOnTop === 'boolean' ? companion.alwaysOnTop : true,
          openOnHover: typeof companion.openOnHover === 'boolean' ? companion.openOnHover : false,
          collapseSeconds: [8, 15, 30, 60].includes(Number(companion.collapseSeconds))
            ? Number(companion.collapseSeconds)
            : 15,
          coordinates,
          freeMode: typeof companion.freeMode === 'boolean' ? companion.freeMode : false,
          desktopCoordinates,
          sounds: normalizeSoundPreferences(companion.sounds),
        },
      },
      warnings,
    };
  } catch {
    return { settings: defaultSettings, warnings };
  }
}

function providerEndpointHostAllowed(endpoint: string, hostRule: string): boolean {
  try {
    const url = new URL(endpoint);
    if (url.protocol !== 'https:' || url.username || url.password) return false;
    return hostRule.split('|').some((rule) => {
      const normalized = rule.toLowerCase();
      const host = url.hostname.toLowerCase();
      if (normalized.startsWith('*.')) {
        const suffix = normalized.slice(1);
        return host.endsWith(suffix) && host.length > suffix.length;
      }
      if (normalized.includes('{region}')) {
        const expected = normalized.split('.');
        const actual = host.split('.');
        return (
          expected.length === actual.length &&
          expected.every((part, index) =>
            part === '{region}' ? /^[a-z0-9-]+$/.test(actual[index]) : part === actual[index],
          )
        );
      }
      return host === normalized;
    });
  } catch {
    return false;
  }
}

export function saveSettings(settings: AppSettings): boolean {
  // Deliberately persist preferences only. API keys must never enter localStorage.
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(settings));
    return true;
  } catch {
    return false;
  }
}

function isAppSettings(value: unknown): value is AppSettings {
  if (!value || typeof value !== 'object') return false;
  const candidate = value as Partial<AppSettings> & { provider?: Partial<ProviderSettings> };
  return (
    (candidate.language === 'pt-BR' || candidate.language === 'en') &&
    typeof candidate.reduceMotion === 'boolean' &&
    ((!!candidate.providers && typeof candidate.providers === 'object') ||
      (!!candidate.provider &&
        typeof candidate.provider.baseUrl === 'string' &&
        typeof candidate.provider.model === 'string' &&
        typeof candidate.provider.name === 'string'))
  );
}

function validTest(value: unknown): value is ProviderSettings['lastTest'] {
  if (!value || typeof value !== 'object') return false;
  const test = value as ProviderSettings['lastTest'];
  return (
    ['passed', 'failed', 'partial'].includes(test?.status ?? '') &&
    typeof test?.at === 'string' &&
    typeof test?.message === 'string'
  );
}
