import { providerById, type AiProvider } from './providers';
import { defaultProviderSettings, type AppSettings } from './settings';

export type ProviderDirectoryTab = 'connected' | 'free' | 'all';
export const providerCardHasManagement = (tab: ProviderDirectoryTab, connected: boolean): boolean =>
  tab === 'connected' && connected;
const PROVIDER_TAB_STORAGE_KEY = 'odekrai.provider-directory.tab';

export function loadProviderDirectoryTab(storage?: Pick<Storage, 'getItem'>): ProviderDirectoryTab {
  try {
    const value = storage?.getItem(PROVIDER_TAB_STORAGE_KEY);
    return value === 'connected' || value === 'free' || value === 'all' ? value : 'all';
  } catch {
    return 'all';
  }
}

export function saveProviderDirectoryTab(
  tab: ProviderDirectoryTab,
  storage?: Pick<Storage, 'setItem'>,
): void {
  try {
    storage?.setItem(PROVIDER_TAB_STORAGE_KEY, tab);
  } catch {
    // A aba continua funcional quando o armazenamento do navegador está indisponível.
  }
}

export async function loadSavedCredentialProviderIds(
  providerIds: readonly string[],
  hasCredential: (providerId: string) => Promise<boolean>,
): Promise<{ credentialIds: Set<string>; failedIds: Set<string> }> {
  const credentialIds = new Set<string>();
  const failedIds = new Set<string>();
  for (const providerId of providerIds) {
    try {
      if (await hasCredential(providerId)) credentialIds.add(providerId);
    } catch {
      failedIds.add(providerId);
    }
  }
  return { credentialIds, failedIds };
}

export function filterProviderDirectory(
  providers: AiProvider[],
  tab: ProviderDirectoryTab,
  search: string,
  connectedIds: ReadonlySet<string>,
): AiProvider[] {
  const query = search.trim().toLocaleLowerCase('pt-BR');
  return providers.filter((provider) => {
    if (tab === 'connected' && !connectedIds.has(provider.id)) return false;
    if (tab === 'free' && provider.freePlan?.status !== 'verified') return false;
    return !query || provider.name.toLocaleLowerCase('pt-BR').includes(query);
  });
}

export function isProviderConnected(
  providerId: string,
  credentialSaved: boolean,
  lastTestStatus: 'passed' | 'failed' | 'partial' | undefined,
  localServiceOnline: boolean,
): boolean {
  if (providerId === 'local') return localServiceOnline;
  return credentialSaved && lastTestStatus === 'passed';
}

export function setProviderEnabled<T extends { enabled?: boolean }>(
  settings: T,
  enabled: boolean,
): T & { enabled: boolean } {
  return { ...settings, enabled };
}

export function setPrimaryProviderSettings(settings: AppSettings, providerId: string): AppSettings {
  const provider = settings.providers[providerId] ?? defaultProviderSettings(providerId);
  return {
    ...settings,
    activeProviderId: providerId,
    fallbackProviderIds: settings.fallbackProviderIds.filter((id) => id !== providerId),
    providers: { ...settings.providers, [providerId]: { ...provider, enabled: true } },
  };
}

export function setFallbackProviderIds(settings: AppSettings, providerIds: string[]): AppSettings {
  const unique = [...new Set(providerIds)].filter(
    (providerId) =>
      providerId !== settings.activeProviderId &&
      Boolean(providerById(providerId)) &&
      settings.providers[providerId]?.enabled !== false,
  );
  return { ...settings, fallbackProviderIds: unique };
}

export function moveFallbackProvider(
  settings: AppSettings,
  providerId: string,
  offset: -1 | 1,
): AppSettings {
  const from = settings.fallbackProviderIds.indexOf(providerId);
  const to = from + offset;
  if (from < 0 || to < 0 || to >= settings.fallbackProviderIds.length) return settings;
  const fallbackProviderIds = [...settings.fallbackProviderIds];
  [fallbackProviderIds[from], fallbackProviderIds[to]] = [
    fallbackProviderIds[to],
    fallbackProviderIds[from],
  ];
  return { ...settings, fallbackProviderIds };
}

export function toggleProviderSettings(
  settings: AppSettings,
  providerId: string,
  enabled: boolean,
): AppSettings {
  const provider = settings.providers[providerId] ?? defaultProviderSettings(providerId);
  return {
    ...settings,
    activeProviderId:
      !enabled && settings.activeProviderId === providerId ? '' : settings.activeProviderId,
    fallbackProviderIds: enabled
      ? settings.fallbackProviderIds
      : settings.fallbackProviderIds.filter((id) => id !== providerId),
    providers: { ...settings.providers, [providerId]: { ...provider, enabled } },
  };
}

export function removeSavedCredentialProviderId(
  credentialIds: ReadonlySet<string>,
  providerId: string,
): Set<string> {
  const next = new Set(credentialIds);
  next.delete(providerId);
  return next;
}

export async function removeProviderCredential(
  providerId: string,
  remove: (providerId: string) => Promise<unknown>,
  credentialIds: ReadonlySet<string>,
): Promise<Set<string>> {
  await remove(providerId);
  return removeSavedCredentialProviderId(credentialIds, providerId);
}

export async function confirmProviderCredentialRemoval(
  providerName: string,
  confirm: (message: string) => Promise<boolean>,
  remove: () => Promise<void>,
): Promise<boolean> {
  if (!(await confirm(`Remover a credencial salva de ${providerName}?`))) return false;
  await remove();
  return true;
}
