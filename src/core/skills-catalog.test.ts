import { describe, expect, it, vi } from 'vitest';
import {
  familyEnableTarget,
  displaySkillFamily,
  filterSkills,
  groupSkillFamilies,
  nextSkillsTab,
  setSkillEnabledCommand,
  setSkillGroupEnabledCommand,
  setSkillOverrideCommand,
  type CatalogSkill,
} from './skills-catalog';

const skills: CatalogSkill[] = [
  {
    id: 'files.read',
    name: 'Ler arquivo',
    description: 'Lê arquivos permitidos',
    risk: 'leitura',
    resultTrust: 'local',
    enabled: false,
    morePermissive: false,
    fixedBlocked: false,
    category: 'local',
    status: 'available',
    configurationRequired: false,
    family: 'arquivos',
    icon: 'folder',
    examples: ['Leia o relatório da pasta permitida'],
    origin: { kind: 'builtin' },
  },
  {
    id: 'workspace.write_file',
    name: 'Salvar arquivo',
    description: 'Grava texto com cópia reversível',
    risk: 'escrita_reversivel',
    resultTrust: 'local',
    enabled: true,
    morePermissive: false,
    fixedBlocked: false,
    category: 'local',
    status: 'installed',
    configurationRequired: false,
    family: 'arquivos',
    icon: 'folder',
    examples: ['Salve este resumo no arquivo notas.txt'],
    origin: { kind: 'builtin' },
  },
  {
    id: 'discord.send',
    name: 'Enviar ao Discord',
    description: 'Envia mensagens por webhook',
    risk: 'comunicacao_externa',
    resultTrust: 'local',
    enabled: false,
    morePermissive: false,
    fixedBlocked: true,
    category: 'integration',
    status: 'configuration_required',
    configurationRequired: true,
    family: 'mensagens',
    icon: 'chat_text',
    examples: ['Envie a mensagem aprovada ao canal'],
    origin: { kind: 'builtin' },
  },
  {
    id: 'theme.set',
    name: 'Alterar tema do Windows',
    description: 'Experimental: altera entre claro e escuro e permite desfazer.',
    risk: 'escrita_reversivel',
    resultTrust: 'local',
    enabled: false,
    morePermissive: false,
    fixedBlocked: false,
    category: 'local',
    status: 'available',
    configurationRequired: false,
    family: 'computador',
    icon: 'desktop',
    examples: ['Altere o tema do Windows para escuro'],
    origin: { kind: 'builtin' },
  },
];

describe('catálogo de habilidades', () => {
  it('agrupa somente as famílias e habilidades recebidas do registro', () => {
    const families = groupSkillFamilies(skills);
    expect(families.map(({ family }) => family)).toEqual(['arquivos', 'mensagens', 'computador']);
    expect(families.flatMap(({ skills: members }) => members.map(({ id }) => id))).toEqual(
      skills.map(({ id }) => id),
    );
    expect(families[0].activeCount).toBe(1);
    expect(families[0].highestRisk).toBe('escrita_reversivel');
    expect(families[1].status).toBe('configuration_required');
    const withUnavailable = groupSkillFamilies([
      ...skills.slice(0, 2),
      {
        ...skills[0],
        id: 'simulated.payment',
        risk: 'destrutivo_financeiro',
        status: 'unavailable',
      },
    ]);
    expect(withUnavailable[0].highestRisk).toBe('escrita_reversivel');
  });

  it('consolida serviços externos e habilidades do sistema nas famílias visíveis do produto', () => {
    expect(displaySkillFamily({ id: 'app.launch', family: 'abrir_aplicativos' })).toBe('sistema');
    expect(displaySkillFamily({ id: 'system.info', family: 'computador' })).toBe('sistema');
    expect(displaySkillFamily({ id: 'google.calendar.list_events', family: 'google' })).toBe(
      'integracoes',
    );
    expect(displaySkillFamily({ id: 'shardcloud.vm_list', family: 'nuvem' })).toBe('integracoes');
    expect(displaySkillFamily({ id: 'mcp.server.tool', family: 'integracoes_mcp' })).toBe(
      'integracoes',
    );
  });

  it('procura por nome, descrição, exemplo e família sem diferenciar caixa', () => {
    expect(filterSkills(skills, 'ARQUIVOS').map(({ id }) => id)).toEqual([
      'files.read',
      'workspace.write_file',
    ]);
    expect(filterSkills(skills, 'relatório da pasta').map(({ id }) => id)).toEqual(['files.read']);
    expect(filterSkills(skills, 'Grava texto').map(({ id }) => id)).toEqual([
      'workspace.write_file',
    ]);
    expect(filterSkills(skills, 'discord.send')).toHaveLength(1);
    expect(filterSkills(skills, 'inexistente')).toEqual([]);
  });

  it('percorre as abas com setas e extremos do teclado', () => {
    expect(nextSkillsTab('pessoal', 'ArrowRight')).toBe('publico');
    expect(nextSkillsTab('publico', 'ArrowLeft')).toBe('pessoal');
    expect(nextSkillsTab('publico', 'Home')).toBe('pessoal');
    expect(nextSkillsTab('pessoal', 'End')).toBe('publico');
    expect(nextSkillsTab('pessoal', 'Enter')).toBeNull();
  });

  it('calcula o interruptor de família sem ativar itens que exigem configuração ou estão indisponíveis', () => {
    expect(familyEnableTarget(skills.slice(0, 2))).toBe(true);
    expect(familyEnableTarget([skills[2]])).toBeUndefined();
    expect(
      familyEnableTarget([
        { ...skills[0], status: 'unavailable', enabled: false },
        { ...skills[2], status: 'available', configurationRequired: false },
      ]),
    ).toBe(true);
  });

  it('envia ativações individuais e por família aos comandos Rust com parâmetros fechados', async () => {
    const command = vi.fn(async (_name: string, _args?: Record<string, unknown>) => undefined);
    await setSkillEnabledCommand('files.read', true, command);
    await setSkillEnabledCommand('files.read', false, command);
    await setSkillGroupEnabledCommand('arquivos', false, command);
    await setSkillGroupEnabledCommand('local', true, command);
    await setSkillOverrideCommand('files.write', true, command);
    expect(command.mock.calls).toEqual([
      ['set_skill_enabled', { skillId: 'files.read', enabled: true }],
      ['set_skill_enabled', { skillId: 'files.read', enabled: false }],
      ['set_skill_group_enabled', { category: 'arquivos', enabled: false }],
      ['set_skill_group_enabled', { category: 'local', enabled: true }],
      ['set_tool_override', { tool: 'files.write', morePermissive: true }],
    ]);
  });
});
