import { describe, expect, it, vi } from 'vitest';
import {
  applyHomeSuggestion,
  createHomeChecklist,
  createHomeSuggestions,
  getTodayBounds,
  submitHomeMessage,
} from './home';
import type { HomeContext } from './home';

const emptyContext = (): HomeContext => ({
  activeSkills: [],
  agentAvailable: false,
  todayEvents: [],
  reminderCount: 0,
  pendingApprovalCount: 0,
  recentConversations: [],
});

describe('início do OdeKrai', () => {
  it('produz sugestões de agenda e lembrete somente com habilidades e dados disponíveis', () => {
    const context = emptyContext();
    context.agentAvailable = true;
    context.activeSkills = [
      { id: 'calendar.list_events', family: 'agenda_lembretes', name: 'Listar eventos' },
      { id: 'reminder.list', family: 'agenda_lembretes', name: 'Listar lembretes' },
    ];
    context.todayEvents = [{ id: 'event-1', title: 'Consulta', start: '2030-01-01T10:00:00Z' }];
    context.reminderCount = 2;

    expect(createHomeSuggestions(context).map(({ id }) => id)).toEqual([
      'event-event-1',
      'reminders',
      'agent-task',
    ]);

    context.activeSkills = [];
    expect(createHomeSuggestions(context).map(({ id }) => id)).toEqual(['agent-task']);
    context.activeSkills = [{ id: 'files.search', family: 'arquivos', name: 'Buscar arquivos' }];
    expect(createHomeSuggestions(context).map(({ id }) => id)).toEqual([
      'agent-task',
      'family-arquivos',
    ]);
  });

  it('mantém ordem, usa conversas reais e limita a quatro sugestões', () => {
    const context = emptyContext();
    context.agentAvailable = true;
    context.activeSkills = [
      { id: 'calendar.list_events', family: 'agenda_lembretes', name: 'Listar eventos' },
      { id: 'reminder.list', family: 'agenda_lembretes', name: 'Listar lembretes' },
      { id: 'files.search', family: 'arquivos', name: 'Buscar arquivos' },
      { id: 'doc.create_markdown', family: 'planilhas_documentos', name: 'Criar documento' },
    ];
    context.todayEvents = [{ id: 'event-1', title: 'Planejamento', start: '2030-01-01T10:00:00Z' }];
    context.reminderCount = 1;
    context.recentConversations = [
      {
        id: 'conversation-1',
        title: 'Plano do projeto',
        preview: 'Definir etapas',
        created_at: 100,
      },
    ];

    const suggestions = createHomeSuggestions(context, 4);
    expect(suggestions.map(({ id }) => id)).toEqual([
      'event-event-1',
      'reminders',
      'agent-task',
      'conversation-conversation-1',
    ]);
    expect(suggestions[0].prompt).toContain('Planejamento');
    expect(suggestions[3].prompt).toContain('Plano do projeto');
    expect(createHomeSuggestions(context, 0)).toEqual([]);
  });

  it('não inventa sugestões e não oferece tarefas de agente com aprovações pendentes', () => {
    const context = emptyContext();
    expect(createHomeSuggestions(context)).toEqual([]);
    context.agentAvailable = true;
    context.pendingApprovalCount = 1;
    context.activeSkills = [{ id: 'files.search', family: 'arquivos', name: 'Buscar arquivos' }];
    expect(createHomeSuggestions(context)).toEqual([]);
  });

  it('ignora títulos ausentes em eventos e conversas sem quebrar sugestões', () => {
    const malformed = {
      ...emptyContext(),
      agentAvailable: true,
      activeSkills: [
        { id: 'calendar.list_events', family: 'agenda_lembretes' as const, name: 'Agenda' },
      ],
      todayEvents: [{ id: 'event-1', start: '2030-01-01T10:00:00Z' }],
      recentConversations: [{ id: 'conversation-1', preview: '', created_at: 1 }],
    } as unknown as HomeContext;

    const suggestions = createHomeSuggestions(malformed);
    expect(suggestions.map(({ id }) => id)).not.toContain('event-event-1');
    expect(suggestions.map(({ id }) => id)).not.toContain('conversation-conversation-1');
  });

  it('preenche a mensagem com uma sugestão sem enviar automaticamente', () => {
    const suggestion = createHomeSuggestions({
      ...emptyContext(),
      recentConversations: [{ id: 'c1', title: 'Pesquisa', preview: 'Resumo', created_at: 1 }],
    })[0];
    const setDraft = vi.fn();
    const send = vi.fn();
    applyHomeSuggestion(suggestion, setDraft);
    expect(setDraft).toHaveBeenCalledWith('Quero conversar mais sobre “Pesquisa”.');
    expect(send).not.toHaveBeenCalled();
  });

  it('monta um checklist apenas para requisitos ausentes e usa destinos de navegação reais', () => {
    expect(
      createHomeChecklist({
        providerReady: false,
        workspaceAvailable: false,
        agentAvailable: false,
        agentReason: 'Configure a pasta de trabalho do agente.',
      }).map(({ id, destination }) => [id, destination]),
    ).toEqual([
      ['provider', 'providers'],
      ['workspace', 'agent'],
    ]);
    expect(
      createHomeChecklist({
        providerReady: true,
        workspaceAvailable: true,
        agentAvailable: false,
        agentReason: 'Este modelo não tem suporte verificado a ferramentas.',
      })[0],
    ).toMatchObject({ id: 'agent-tools', destination: 'providers' });
    expect(
      createHomeChecklist({
        providerReady: true,
        workspaceAvailable: true,
        agentAvailable: true,
        agentReason: null,
      }),
    ).toEqual([]);
  });

  it('abre uma conversa e envia somente após envio explícito com texto', async () => {
    const start = vi.fn();
    const send = vi.fn(async () => undefined);
    expect(await submitHomeMessage('  Olá, OdeKrai  ', false, start, send)).toBe(true);
    expect(start).toHaveBeenCalledOnce();
    expect(send).toHaveBeenCalledWith('Olá, OdeKrai');
    expect(await submitHomeMessage('  ', false, start, send)).toBe(false);
    expect(await submitHomeMessage('Duplicada', true, start, send)).toBe(false);
    expect(await submitHomeMessage('Sem provedor', false, start, send, false)).toBe(false);
    expect(start).toHaveBeenCalledOnce();
    expect(send).toHaveBeenCalledOnce();
  });

  it('calcula limites de hoje no fuso local informado', () => {
    const bounds = getTodayBounds(new Date(2030, 0, 2, 12));
    expect(new Date(bounds.from).getDate()).toBe(2);
    expect(new Date(bounds.from).getHours()).toBe(0);
    expect(new Date(bounds.to).getDate()).toBe(3);
    expect(new Date(bounds.to).getHours()).toBe(0);
  });
});
