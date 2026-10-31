import type { Section } from '../ui/screens/types';

export type Page = 'overview' | 'chat' | 'calendar' | 'skills' | 'providers';
export type SettingsSection =
  | 'geral'
  | 'perfil-memoria'
  | 'assistente'
  | 'computador'
  | 'habilidades'
  | 'conectores'
  | 'privacidade'
  | 'atividade'
  | 'nuvem'
  | 'sobre';

export type AppRoute = { page: Page; settingsSection?: SettingsSection };

export const LEGACY_SETTINGS_SECTIONS: Partial<Record<Section, SettingsSection>> = {
  agent: 'perfil-memoria',
  companion: 'assistente',
  settings: 'geral',
  permissions: 'privacidade',
  activity: 'atividade',
  computer: 'computador',
};

export interface NavigationSnapshot {
  route: AppRoute;
  section: Section;
}

export interface NavigationView extends NavigationSnapshot {
  settingsOpen: boolean;
}

export function resolveLegacySettingsNavigation(
  destination: Section,
  current: NavigationView,
): NavigationView | null {
  const settingsSection = LEGACY_SETTINGS_SECTIONS[destination];
  if (!settingsSection) return null;
  return {
    route: { page: current.route.page, settingsSection },
    section: current.section,
    settingsOpen: true,
  };
}

export function resolveLegacyScreenNavigation(
  destination: Section,
  current: NavigationView,
): NavigationView {
  return {
    route: { page: current.route.page },
    section: destination,
    settingsOpen: false,
  };
}

export function resolveNavigation(destination: Section | AppRoute): NavigationSnapshot {
  if (typeof destination === 'string') {
    const settingsSection = LEGACY_SETTINGS_SECTIONS[destination];
    const page = isPage(destination) ? destination : 'overview';
    return {
      route: settingsSection ? { page, settingsSection } : { page },
      section: destination,
    };
  }

  const section = destination.settingsSection
    ? ((Object.entries(LEGACY_SETTINGS_SECTIONS).find(
        ([, value]) => value === destination.settingsSection,
      )?.[0] as Section | undefined) ?? 'settings')
    : destination.page;
  return { route: { ...destination }, section };
}

export function resolveAgentReason(reason: string): Section {
  if (reason.includes('pasta') || reason.includes('identidade')) return 'agent';
  if (reason.includes('habilidade')) return 'skills';
  if (reason.includes('parada de emergência')) return 'permissions';
  return 'providers';
}

export function resolveConfigurationReason(reason: string): Section {
  const normalized = reason.toLocaleLowerCase('pt-BR');
  if (normalized.includes('identidade') || normalized.includes('pasta do odekrai')) return 'agent';
  if (normalized.includes('habilidade')) return 'skills';
  if (normalized.includes('limite diário')) return 'settings';
  return 'providers';
}

export function isPage(value: string): value is Page {
  return ['overview', 'chat', 'calendar', 'skills', 'providers'].includes(value);
}

export function readLastPage(storage?: Pick<Storage, 'getItem'>): Page {
  try {
    const value = (storage ?? window.localStorage).getItem('odekrai:last-page');
    return value && isPage(value) ? value : 'overview';
  } catch {
    return 'overview';
  }
}

export function readLastSettingsSection(storage?: Pick<Storage, 'getItem'>): SettingsSection {
  try {
    const value = (storage ?? window.localStorage).getItem('odekrai:last-settings-section');
    return SETTINGS_SECTIONS.includes(value as SettingsSection)
      ? (value as SettingsSection)
      : 'geral';
  } catch {
    return 'geral';
  }
}

export const SETTINGS_SECTIONS: SettingsSection[] = [
  'geral',
  'perfil-memoria',
  'assistente',
  'computador',
  'habilidades',
  'conectores',
  'privacidade',
  'atividade',
  'nuvem',
  'sobre',
];

export function persistSettingsSection(
  section: SettingsSection,
  storage?: Pick<Storage, 'setItem'>,
): void {
  try {
    (storage ?? window.localStorage).setItem('odekrai:last-settings-section', section);
  } catch {
    // A seção continua selecionada enquanto o aplicativo estiver aberto.
  }
}

export function persistPage(page: Page, storage?: Pick<Storage, 'setItem'>): void {
  try {
    (storage ?? window.localStorage).setItem('odekrai:last-page', page);
  } catch {
    // Navegação segue disponível quando o armazenamento local estiver bloqueado.
  }
}
