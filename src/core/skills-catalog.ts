export type SkillStatus =
  'installed' | 'available' | 'blocked' | 'configuration_required' | 'unavailable';
export type DiscordSkillState = 'not_configured' | 'active' | 'blocked';
export type SkillCategory = 'local' | 'integration';
export type SkillRisk =
  | 'leitura'
  | 'escrita_reversivel'
  | 'comunicacao_externa'
  | 'destrutivo_financeiro'
  | 'execucao_de_codigo'
  | 'captura_sensivel';
export type SkillFamily =
  | 'arquivos'
  | 'abrir_aplicativos'
  | 'agenda_lembretes'
  | 'planilhas_documentos'
  | 'mensagens'
  | 'web'
  | 'computador'
  | 'codigo'
  | 'nuvem'
  | 'integracoes_mcp';
export type SkillOrigin = { kind: 'builtin' } | { kind: 'mcp'; server: string };
export type SkillsCatalogTab = 'pessoal' | 'publico';

export function nextSkillsTab(current: SkillsCatalogTab, key: string): SkillsCatalogTab | null {
  if (key === 'Home') return 'pessoal';
  if (key === 'End') return 'publico';
  if (key === 'ArrowRight') return current === 'pessoal' ? 'publico' : 'pessoal';
  if (key === 'ArrowLeft') return current === 'publico' ? 'pessoal' : 'publico';
  return null;
}

export type CatalogSkill = {
  id: string;
  name: string;
  description: string;
  risk: SkillRisk;
  resultTrust: 'local' | 'untrusted';
  enabled: boolean;
  morePermissive: boolean;
  fixedBlocked: boolean;
  category: SkillCategory;
  status: SkillStatus;
  statusReason?: string | null;
  configurationRequired: boolean;
  discordState?: DiscordSkillState;
  family: SkillFamily;
  icon: string;
  examples: string[];
  origin: SkillOrigin;
};

export type SkillFamilyPresentation = {
  family: SkillFamily;
  name: string;
  description: string;
};

export const skillFamilyPresentation: Record<
  SkillFamily,
  Omit<SkillFamilyPresentation, 'family'>
> = {
  arquivos: {
    name: 'Arquivos',
    description: 'Leitura, organização e gravação em locais permitidos.',
  },
  abrir_aplicativos: {
    name: 'Abertura e aplicativos',
    description: 'Abertura de aplicativos cadastrados pelo usuário.',
  },
  agenda_lembretes: {
    name: 'Agenda e lembretes',
    description: 'Eventos e lembretes locais ou integrações de calendário disponíveis.',
  },
  planilhas_documentos: {
    name: 'Planilhas e documentos',
    description: 'Criação e leitura de documentos e planilhas locais.',
  },
  mensagens: {
    name: 'Mensagens',
    description: 'Rascunhos e comunicações externas registradas no aplicativo.',
  },
  web: { name: 'Web', description: 'Leitura de páginas e abertura de endereços web.' },
  computador: {
    name: 'Computador',
    description: 'Controles e informações disponíveis neste computador.',
  },
  codigo: { name: 'Código', description: 'Execução assistida de tarefas de código.' },
  nuvem: { name: 'Nuvem', description: 'Consulta e gerenciamento aprovado das VMs conectadas.' },
  integracoes_mcp: {
    name: 'Integrações MCP',
    description: 'Ferramentas fornecidas por servidores MCP cadastrados.',
  },
};

export type CatalogFamily = SkillFamilyPresentation & {
  icon: string;
  skills: CatalogSkill[];
  activeCount: number;
  status: 'active' | 'disabled' | 'configuration_required' | 'unavailable';
  highestRisk: SkillRisk;
};

export type SkillCommand = (command: string, args?: Record<string, unknown>) => Promise<unknown>;

const riskRank: Record<SkillRisk, number> = {
  leitura: 0,
  escrita_reversivel: 1,
  comunicacao_externa: 2,
  execucao_de_codigo: 3,
  destrutivo_financeiro: 4,
  captura_sensivel: 5,
};

export async function setSkillEnabledCommand(
  skillId: string,
  enabled: boolean,
  command: SkillCommand,
): Promise<void> {
  await command('set_skill_enabled', { skillId, enabled });
}

export async function setSkillGroupEnabledCommand(
  family: SkillFamily | SkillCategory,
  enabled: boolean,
  command: SkillCommand,
): Promise<void> {
  await command('set_skill_group_enabled', { category: family, enabled });
}

export async function setSkillOverrideCommand(
  skillId: string,
  morePermissive: boolean,
  command: SkillCommand,
): Promise<void> {
  await command('set_tool_override', { tool: skillId, morePermissive });
}

export function filterSkills<T extends CatalogSkill>(skills: T[], query: string): T[] {
  const normalized = query.trim().toLocaleLowerCase('pt-BR');
  if (!normalized) return skills;
  return skills.filter((skill) =>
    [
      skill.name,
      skill.description,
      skill.id,
      skillFamilyPresentation[skill.family].name,
      ...skill.examples,
    ]
      .join(' ')
      .toLocaleLowerCase('pt-BR')
      .includes(normalized),
  );
}

export function groupSkillFamilies(skills: CatalogSkill[]): CatalogFamily[] {
  const families = new Map<SkillFamily, CatalogSkill[]>();
  for (const skill of skills) {
    const members = families.get(skill.family) ?? [];
    members.push(skill);
    families.set(skill.family, members);
  }

  return [...families].map(([family, members]) => {
    const activeCount = members.filter((skill) => skill.enabled).length;
    const allUnavailable = members.every((skill) => skill.status === 'unavailable');
    const hasConfigurationRequirement = members.some(
      (skill) => skill.status === 'configuration_required' && !skill.enabled,
    );
    const status =
      activeCount > 0
        ? 'active'
        : allUnavailable
          ? 'unavailable'
          : hasConfigurationRequirement
            ? 'configuration_required'
            : 'disabled';
    const riskBearingSkills = members.filter((skill) => skill.status !== 'unavailable');
    const skillsWithRisk = riskBearingSkills.length > 0 ? riskBearingSkills : members;
    const highestRisk = skillsWithRisk
      .slice(1)
      .reduce(
        (highest, skill) => (riskRank[skill.risk] > riskRank[highest] ? skill.risk : highest),
        skillsWithRisk[0].risk,
      );

    return {
      family,
      ...skillFamilyPresentation[family],
      icon: members[0].icon,
      skills: members,
      activeCount,
      status,
      highestRisk,
    };
  });
}

export function familyEnableTarget(skills: CatalogSkill[]): boolean | undefined {
  const eligible = skills.filter(
    (skill) =>
      skill.status === 'available' || skill.status === 'blocked' || skill.status === 'installed',
  );
  if (eligible.length === 0) return undefined;
  return !eligible.every((skill) => skill.enabled);
}

export function skillStatusLabel(status: CatalogFamily['status']): string {
  switch (status) {
    case 'active':
      return 'Ativa';
    case 'disabled':
      return 'Desativada';
    case 'configuration_required':
      return 'Não configurada';
    case 'unavailable':
      return 'Indisponível';
  }
}
