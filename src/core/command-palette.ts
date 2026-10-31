import type { Page, SettingsSection } from './navigation';
import { isPage, SETTINGS_SECTIONS } from './navigation';
import { SETTINGS_INDEX, SETTINGS_MENU } from './settings-navigation';
import { skillFamilyPresentation, type SkillFamily } from './skills-catalog';
import { PETS } from './pets';

export type PermissionMode = 'sempre_perguntar' | 'aprovar_por_mim' | 'controle_total';
export type PaletteAction =
  | { type: 'page'; page: Page }
  | { type: 'settings'; section: SettingsSection; anchor?: string }
  | { type: 'new-conversation' }
  | { type: 'conversation'; conversationId: string }
  | { type: 'agent-mode'; enabled: boolean }
  | { type: 'skill-family'; family: SkillFamily }
  | { type: 'pet'; petId: string }
  | { type: 'permission-mode'; mode: PermissionMode }
  | { type: 'stop-all' }
  | { type: 'mute' };

export type PaletteItem = {
  id: string;
  title: string;
  keywords: readonly string[];
  shortcut?: string;
  action: PaletteAction;
};

export type PaletteActionHandlers = {
  page: (page: Page) => void;
  settings: (section: SettingsSection, anchor?: string) => void;
  newConversation: () => void;
  conversation: (id: string) => void;
  agentMode: (enabled: boolean) => void;
  skillFamily: (family: SkillFamily) => void;
  pet: (id: string) => void;
  permissionMode: (mode: PermissionMode) => void;
  stopAll: () => void;
  mute: () => void;
};

export function dispatchPaletteAction(
  action: PaletteAction,
  handlers: PaletteActionHandlers,
): void {
  switch (action.type) {
    case 'page':
      handlers.page(action.page);
      break;
    case 'settings':
      handlers.settings(action.section, action.anchor);
      break;
    case 'new-conversation':
      handlers.newConversation();
      break;
    case 'conversation':
      handlers.conversation(action.conversationId);
      break;
    case 'agent-mode':
      handlers.agentMode(action.enabled);
      break;
    case 'skill-family':
      handlers.skillFamily(action.family);
      break;
    case 'pet':
      handlers.pet(action.petId);
      break;
    case 'permission-mode':
      handlers.permissionMode(action.mode);
      break;
    case 'stop-all':
      handlers.stopAll();
      break;
    case 'mute':
      handlers.mute();
      break;
  }
}

const PAGES: readonly { page: Page; title: string; keywords: readonly string[] }[] = [
  { page: 'overview', title: 'Ir para Início', keywords: ['home', 'começar'] },
  { page: 'chat', title: 'Ir para Conversas', keywords: ['chat', 'mensagens'] },
  { page: 'calendar', title: 'Ir para Agenda', keywords: ['calendário', 'eventos'] },
  { page: 'skills', title: 'Ir para Habilidades', keywords: ['ferramentas'] },
  { page: 'providers', title: 'Ir para Provedores', keywords: ['conectores', 'ia'] },
];

const PERMISSION_MODES: readonly {
  mode: PermissionMode;
  title: string;
  keywords: readonly string[];
}[] = [
  {
    mode: 'sempre_perguntar',
    title: 'Modo de permissão: Sempre perguntar',
    keywords: ['permissões', 'segurança'],
  },
  {
    mode: 'aprovar_por_mim',
    title: 'Modo de permissão: Aprovar por mim',
    keywords: ['permissões', 'segurança'],
  },
  {
    mode: 'controle_total',
    title: 'Modo de permissão: Controle total',
    keywords: ['permissões', 'segurança'],
  },
];

export const COMMAND_PALETTE_ITEMS: readonly PaletteItem[] = [
  ...PAGES.map(({ page, title, keywords }) => ({
    id: `page:${page}`,
    title,
    keywords,
    action: { type: 'page', page } as const,
  })),
  ...SETTINGS_MENU.map(({ id, title }) => ({
    id: `settings:${id}`,
    title: `Configurações: ${title}`,
    keywords: ['configurações', 'preferências'],
    action: { type: 'settings', section: id } as const,
  })),
  ...SETTINGS_INDEX.map(({ title, keywords, section, anchor }) => ({
    id: `setting:${anchor}`,
    title: `Configurações: ${title}`,
    keywords,
    action: { type: 'settings', section, anchor } as const,
  })),
  {
    id: 'conversation:new',
    title: 'Nova conversa',
    keywords: ['chat', 'mensagem'],
    action: { type: 'new-conversation' } as const,
  },
  ...PERMISSION_MODES.map(({ mode, title, keywords }) => ({
    id: `permission:${mode}`,
    title,
    keywords,
    action: { type: 'permission-mode', mode } as const,
  })),
  {
    id: 'app:stop-all',
    title: 'Parar tudo',
    keywords: ['parada', 'emergência', 'agente'],
    action: { type: 'stop-all' } as const,
  },
  {
    id: 'pet:mute',
    title: 'Silenciar',
    keywords: ['som', 'áudio', 'pet'],
    action: { type: 'mute' } as const,
  },
].map((item) => item);

export function buildPaletteItems(input: {
  conversations: readonly { id: string; title: string }[];
  currentConversationId: string | null;
  agentEnabled: boolean;
}): PaletteItem[] {
  const dynamic: PaletteItem[] = [
    ...input.conversations.map((conversation) => ({
      id: `conversation:${conversation.id}`,
      title: `Abrir conversa: ${conversation.title}`,
      keywords: ['conversa', 'histórico'],
      action: { type: 'conversation' as const, conversationId: conversation.id },
    })),
    ...Object.entries(skillFamilyPresentation).map(([family, presentation]) => ({
      id: `skill:${family}`,
      title: `Habilidades: ${presentation.name}`,
      keywords: [presentation.description, 'família'],
      action: { type: 'skill-family' as const, family: family as SkillFamily },
    })),
    ...PETS.map((pet) => ({
      id: `pet:${pet.id}`,
      title: `Trocar mascote: ${pet.name}`,
      keywords: ['personagem', 'avatar', pet.name],
      action: { type: 'pet' as const, petId: pet.id },
    })),
  ];
  if (input.currentConversationId) {
    dynamic.push({
      id: `agent:${input.agentEnabled ? 'off' : 'on'}`,
      title: input.agentEnabled
        ? 'Desligar modo agente da conversa'
        : 'Ligar modo agente da conversa',
      keywords: ['agente', 'modo'],
      action: { type: 'agent-mode', enabled: !input.agentEnabled },
    });
  }
  return [...COMMAND_PALETTE_ITEMS, ...dynamic];
}

function normalize(value: string): string {
  return value
    .normalize('NFD')
    .replace(/[\u0300-\u036f]/g, '')
    .toLocaleLowerCase('pt-BR')
    .trim();
}

function fuzzyMatch(query: string, candidate: string): number | null {
  if (candidate.includes(query)) return candidate.startsWith(query) ? 0 : 1;
  let position = 0;
  for (const char of query) {
    position = candidate.indexOf(char, position);
    if (position < 0) return null;
    position += 1;
  }
  return 2 + candidate.length / 100;
}

export function searchPaletteItems(
  items: readonly PaletteItem[],
  query: string,
  recentIds: readonly string[] = [],
): PaletteItem[] {
  const needle = normalize(query);
  if (!needle) {
    const rank = new Map(recentIds.map((id, index) => [id, index]));
    return [...items].sort((a, b) => (rank.get(a.id) ?? 10000) - (rank.get(b.id) ?? 10000));
  }
  return items
    .map((item, index) => ({
      item,
      index,
      score: Math.min(
        ...[item.title, ...item.keywords]
          .map((value) => fuzzyMatch(needle, normalize(value)))
          .filter((score): score is number => score !== null),
        Infinity,
      ),
    }))
    .filter(({ score }) => Number.isFinite(score))
    .sort((a, b) => a.score - b.score || a.index - b.index)
    .map(({ item }) => item);
}

export function isValidPaletteDestination(item: PaletteItem): boolean {
  const action = item.action;
  switch (action.type) {
    case 'page':
      return isPage(action.page);
    case 'settings':
      return (
        SETTINGS_SECTIONS.includes(action.section) &&
        (!action.anchor ||
          SETTINGS_INDEX.some(
            (entry) => entry.anchor === action.anchor && entry.section === action.section,
          ))
      );
    case 'skill-family':
      return action.family in skillFamilyPresentation;
    case 'pet':
      return PETS.some((pet) => pet.id === action.petId);
    default:
      return true;
  }
}

export function updateRecentCommands(
  recentIds: readonly string[],
  id: string,
  limit = 8,
): string[] {
  return [id, ...recentIds.filter((recent) => recent !== id)].slice(0, limit);
}

export function nextPaletteIndex(
  key: string,
  currentIndex: number,
  resultCount: number,
): number | null {
  if (!resultCount || !['ArrowDown', 'ArrowUp'].includes(key)) return null;
  const direction = key === 'ArrowDown' ? 1 : -1;
  return (currentIndex + direction + resultCount) % resultCount;
}
