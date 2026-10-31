import type { SettingsSection } from './navigation';

export const SETTINGS_MENU = [
  { id: 'geral', title: 'Geral' },
  { id: 'perfil-memoria', title: 'Perfil e memória' },
  { id: 'assistente', title: 'Assistente e mascote' },
  { id: 'computador', title: 'Seu computador' },
  { id: 'habilidades', title: 'Habilidades' },
  { id: 'conectores', title: 'Conectores de IA' },
  { id: 'privacidade', title: 'Privacidade e permissões' },
  { id: 'atividade', title: 'Atividade' },
  { id: 'nuvem', title: 'Nuvem' },
  { id: 'sobre', title: 'Sobre' },
] as const satisfies readonly { id: SettingsSection; title: string }[];

export type SettingsIndexEntry = {
  title: string;
  keywords: string[];
  section: SettingsSection;
  anchor: string;
};

export const SETTINGS_INDEX: readonly SettingsIndexEntry[] = [
  {
    title: 'Idioma da interface',
    keywords: ['português', 'idioma'],
    section: 'geral',
    anchor: 'settings-language',
  },
  {
    title: 'Comportamento da janela',
    keywords: ['fechar', 'bandeja'],
    section: 'geral',
    anchor: 'settings-window',
  },
  {
    title: 'Modo Desempenho',
    keywords: ['automático', 'hardware', 'baixo consumo', 'movimento reduzido'],
    section: 'geral',
    anchor: 'settings-performance',
  },
  {
    title: 'Novidades de novas versões',
    keywords: ['atualizações', 'notas de versão', 'novidades'],
    section: 'geral',
    anchor: 'settings-updates',
  },
  {
    title: 'Aparência e movimento',
    keywords: ['animação', 'movimento reduzido'],
    section: 'geral',
    anchor: 'settings-appearance',
  },
  {
    title: 'Áudio e voz',
    keywords: ['microfone', 'leitura', 'fala'],
    section: 'geral',
    anchor: 'settings-audio',
  },
  {
    title: 'Limites do agente',
    keywords: ['voltas', 'tokens', 'tempo'],
    section: 'geral',
    anchor: 'settings-agent',
  },
  {
    title: 'Links externos confiáveis',
    keywords: ['domínio', 'navegador'],
    section: 'privacidade',
    anchor: 'trusted-links-heading',
  },
  {
    title: 'Pastas permitidas',
    keywords: ['arquivos', 'leitura', 'escrita'],
    section: 'privacidade',
    anchor: 'allowed-folders-heading',
  },
  {
    title: 'Aplicativos permitidos',
    keywords: ['programas', 'executáveis'],
    section: 'privacidade',
    anchor: 'allowed-apps-heading',
  },
  {
    title: 'Destinos do Discord',
    keywords: ['mensagens', 'canal'],
    section: 'privacidade',
    anchor: 'discord-integration-heading',
  },
  {
    title: 'Programas conectados',
    keywords: ['servidores locais', 'integrações'],
    section: 'privacidade',
    anchor: 'mcp-heading',
  },
  {
    title: 'Provedores alternativos',
    keywords: ['reserva', 'fallback'],
    section: 'geral',
    anchor: 'settings-fallbacks',
  },
  {
    title: 'Uso de tokens',
    keywords: ['limite diário', 'consumo'],
    section: 'geral',
    anchor: 'settings-usage',
  },
  {
    title: 'Apagar histórico',
    keywords: ['conversas', 'dados e privacidade'],
    section: 'privacidade',
    anchor: 'permissions-data',
  },
  {
    title: 'Perfil',
    keywords: ['nome', 'apelido', 'avatar'],
    section: 'perfil-memoria',
    anchor: 'identity-profile',
  },
  {
    title: 'Instruções pessoais',
    keywords: ['tom', 'idioma', 'preferências de conversa'],
    section: 'perfil-memoria',
    anchor: 'identity-conversation',
  },
  {
    title: 'Memória local',
    keywords: ['notas', 'pasta pessoal'],
    section: 'perfil-memoria',
    anchor: 'identity-memory',
  },
  {
    title: 'Escolha do mascote',
    keywords: ['personagem', 'avatar'],
    section: 'assistente',
    anchor: 'assistant-pet',
  },
  {
    title: 'Preferências do pet',
    keywords: ['ilha', 'livre', 'posição', 'tamanho'],
    section: 'assistente',
    anchor: 'assistant-behavior',
  },
  {
    title: 'Recursos do computador',
    keywords: ['hardware', 'RAM', 'modelos locais'],
    section: 'computador',
    anchor: 'computer-resources',
  },
  {
    title: 'Estado das habilidades',
    keywords: ['agente', 'ferramentas ativas', 'catálogo'],
    section: 'habilidades',
    anchor: 'skills-summary',
  },
  {
    title: 'Provedor principal e reservas',
    keywords: ['conectores', 'modelo', 'provedor reserva', 'credencial'],
    section: 'conectores',
    anchor: 'connectors-summary',
  },
  {
    title: 'Modo de permissões',
    keywords: ['aprovação', 'controle total'],
    section: 'privacidade',
    anchor: 'permissions-mode',
  },
  {
    title: 'Atalho de parada',
    keywords: ['atalho', 'kill switch'],
    section: 'privacidade',
    anchor: 'permissions-shortcut',
  },
  {
    title: 'Matriz de permissões',
    keywords: ['risco', 'habilidades'],
    section: 'privacidade',
    anchor: 'permissions-matrix',
  },
  {
    title: 'Registro de atividade',
    keywords: ['auditoria', 'eventos', 'log'],
    section: 'atividade',
    anchor: 'activity-log',
  },
  {
    title: 'Execução na nuvem e ShardCloud',
    keywords: ['nuvem', 'cloud', 'shardcloud', 'VM', 'hospedagem', 'execução remota'],
    section: 'nuvem',
    anchor: 'cloud-shardcloud-heading',
  },
  {
    title: 'Informações do aplicativo',
    keywords: ['versão', 'licença', 'build'],
    section: 'sobre',
    anchor: 'settings-about',
  },
];

function normalizeSearch(value: string): string {
  return value
    .normalize('NFD')
    .replace(/[\u0300-\u036f]/g, '')
    .toLocaleLowerCase('pt-BR')
    .trim();
}

export function filterSettingsIndex(query: string): SettingsIndexEntry[] {
  const normalized = normalizeSearch(query);
  if (!normalized) return [...SETTINGS_INDEX];
  return SETTINGS_INDEX.filter((entry) => {
    const sectionTitle = SETTINGS_MENU.find((item) => item.id === entry.section)?.title ?? '';
    return normalizeSearch(`${entry.title} ${entry.keywords.join(' ')} ${sectionTitle}`).includes(
      normalized,
    );
  });
}

export function filterSettingsMenu(query: string): (typeof SETTINGS_MENU)[number][] {
  const normalized = normalizeSearch(query);
  if (!normalized) return [...SETTINGS_MENU];
  const matchingEntries = filterSettingsIndex(query);
  return SETTINGS_MENU.filter(
    (section) =>
      normalizeSearch(section.title).includes(normalized) ||
      matchingEntries.some((entry) => entry.section === section.id),
  );
}

export function settingsMenuLayout(width: number): 'select' | 'vertical' {
  return width < 800 ? 'select' : 'vertical';
}
