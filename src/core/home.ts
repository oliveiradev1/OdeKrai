import { resolveAgentReason } from './navigation';
import type { SkillFamily } from './skills-catalog';
import type { ConversationSummary } from './chat';
import type { Section } from '../ui/screens/types';

export type HomeSkill = { id: string; family: SkillFamily; name: string };
export type HomeEvent = { id: string; title: string; start: string };
export type HomeSuggestionRequirement =
  | { type: 'skill'; id: string }
  | { type: 'skill-family'; family: SkillFamily }
  | { type: 'agent' }
  | { type: 'event'; id: string }
  | { type: 'reminder' }
  | { type: 'no-pending-approvals' }
  | { type: 'conversation'; id: string };

export type HomeSuggestion = {
  id: string;
  label: string;
  prompt: string;
  requirements: HomeSuggestionRequirement[];
};

export type HomeContext = {
  activeSkills: HomeSkill[];
  agentAvailable: boolean;
  todayEvents: HomeEvent[];
  reminderCount: number;
  pendingApprovalCount: number;
  recentConversations: ConversationSummary[];
};

const familyLabels: Record<SkillFamily, string> = {
  arquivos: 'Arquivos',
  abrir_aplicativos: 'Abertura e aplicativos',
  agenda_lembretes: 'Agenda e lembretes',
  planilhas_documentos: 'Planilhas e documentos',
  mensagens: 'Mensagens',
  web: 'Web',
  computador: 'Computador',
  codigo: 'Código',
  nuvem: 'Nuvem',
  integracoes_mcp: 'Integrações MCP',
};

export function getTodayBounds(date: Date): { from: string; to: string } {
  const from = new Date(date.getFullYear(), date.getMonth(), date.getDate());
  const to = new Date(date.getFullYear(), date.getMonth(), date.getDate() + 1);
  return { from: from.toISOString(), to: to.toISOString() };
}

function requirementIsAvailable(requirement: HomeSuggestionRequirement, context: HomeContext) {
  switch (requirement.type) {
    case 'skill':
      return context.activeSkills.some((skill) => skill.id === requirement.id);
    case 'skill-family':
      return context.activeSkills.some((skill) => skill.family === requirement.family);
    case 'agent':
      return context.agentAvailable;
    case 'event':
      return context.todayEvents.some((event) => event.id === requirement.id);
    case 'reminder':
      return context.reminderCount > 0;
    case 'no-pending-approvals':
      return context.pendingApprovalCount === 0;
    case 'conversation':
      return context.recentConversations.some((conversation) => conversation.id === requirement.id);
  }
}

export function createHomeSuggestions(context: HomeContext, limit = 4): HomeSuggestion[] {
  const candidates: HomeSuggestion[] = [];
  const activeSkills = context.activeSkills;
  const canUseAgent = context.agentAvailable;
  const calendarSkill = activeSkills.some(
    (skill) => skill.id === 'calendar.list_events' && skill.family === 'agenda_lembretes',
  );
  const reminderSkill = activeSkills.some(
    (skill) => skill.id === 'reminder.list' && skill.family === 'agenda_lembretes',
  );

  if (calendarSkill) {
    for (const event of context.todayEvents) {
      const title = typeof event.title === 'string' ? event.title : '';
      if (!title.trim()) continue;
      candidates.push({
        id: `event-${event.id}`,
        label: `Preparar: ${title}`,
        prompt: `Ajude-me a me preparar para “${title}”.`,
        requirements: [
          { type: 'skill', id: 'calendar.list_events' },
          { type: 'skill-family', family: 'agenda_lembretes' },
          { type: 'agent' },
          { type: 'event', id: event.id },
        ],
      });
    }
  }

  if (reminderSkill && context.reminderCount > 0) {
    candidates.push({
      id: 'reminders',
      label: 'Revisar lembretes',
      prompt: 'Ajude-me a revisar meus lembretes pendentes.',
      requirements: [
        { type: 'skill', id: 'reminder.list' },
        { type: 'skill-family', family: 'agenda_lembretes' },
        { type: 'agent' },
        { type: 'reminder' },
      ],
    });
  }

  if (canUseAgent && context.pendingApprovalCount === 0) {
    candidates.push({
      id: 'agent-task',
      label: 'Organizar uma tarefa',
      prompt: 'Ajude-me a organizar uma tarefa em etapas.',
      requirements: [{ type: 'agent' }, { type: 'no-pending-approvals' }],
    });
  }

  const recentConversation = context.recentConversations[0];
  const recentTitle = typeof recentConversation?.title === 'string' ? recentConversation.title : '';
  if (recentConversation && recentTitle.trim()) {
    candidates.push({
      id: `conversation-${recentConversation.id}`,
      label: `Explorar: ${recentTitle}`,
      prompt: `Quero conversar mais sobre “${recentTitle}”.`,
      requirements: [{ type: 'conversation', id: recentConversation.id }],
    });
  }

  if (canUseAgent && context.pendingApprovalCount === 0) {
    const families = [...new Set(activeSkills.map((skill) => skill.family))];
    for (const family of families) {
      if (family === 'agenda_lembretes') continue;
      const label = familyLabels[family];
      candidates.push({
        id: `family-${family}`,
        label,
        prompt: `Quero ajuda com ${label.toLocaleLowerCase('pt-BR')}.`,
        requirements: [
          { type: 'skill-family', family },
          { type: 'agent' },
          { type: 'no-pending-approvals' },
        ],
      });
    }
  }

  if (!Number.isFinite(limit) || limit <= 0) return [];
  return candidates
    .filter((candidate) =>
      candidate.requirements.every((requirement) => requirementIsAvailable(requirement, context)),
    )
    .slice(0, Math.floor(limit));
}

export function applyHomeSuggestion(
  suggestion: HomeSuggestion,
  setDraft: (text: string) => void,
): void {
  setDraft(suggestion.prompt);
}

export type HomeChecklistItem = {
  id: 'provider' | 'workspace' | 'agent-tools' | 'agent-skills' | 'agent-security';
  title: string;
  detail: string;
  destination: Section;
};

export function createHomeChecklist(input: {
  providerReady: boolean;
  workspaceAvailable: boolean;
  agentAvailable: boolean;
  agentReason: string | null;
}): HomeChecklistItem[] {
  const items: HomeChecklistItem[] = [];
  if (!input.providerReady) {
    items.push({
      id: 'provider',
      title: 'Conectar um provedor de IA',
      detail: 'Escolha um modelo e salve a credencial com segurança.',
      destination: 'providers',
    });
  }
  if (!input.workspaceAvailable) {
    const identityIncomplete = input.agentReason?.toLocaleLowerCase('pt-BR').includes('identidade');
    items.push({
      id: 'workspace',
      title: identityIncomplete
        ? 'Configurar pasta e identidade do agente'
        : 'Configurar a pasta do OdeKrai',
      detail: 'A pasta de trabalho permite ao agente operar no espaço autorizado.',
      destination: resolveAgentReason(input.agentReason ?? 'Configure a pasta do agente.'),
    });
  } else if (input.providerReady && !input.agentAvailable && input.agentReason) {
    const destination = resolveAgentReason(input.agentReason);
    if (destination === 'providers') {
      items.push({
        id: 'agent-tools',
        title: 'Escolher um provedor com ferramentas',
        detail: 'O modo agente precisa de suporte verificado a ferramentas neste modelo.',
        destination,
      });
    } else if (destination === 'skills') {
      items.push({
        id: 'agent-skills',
        title: 'Ativar uma habilidade',
        detail: 'O modo agente precisa de ao menos uma habilidade disponível e ativa.',
        destination,
      });
    } else if (destination === 'permissions') {
      items.push({
        id: 'agent-security',
        title: 'Retomar o modo agente',
        detail: 'A parada de emergência está ativa.',
        destination,
      });
    }
  }
  return items;
}

export async function submitHomeMessage(
  rawText: string,
  busy: boolean,
  startConversation: () => void,
  send: (text: string) => unknown | Promise<unknown>,
  canSend = true,
): Promise<boolean> {
  const text = rawText.trim();
  if (!text || busy || !canSend) return false;
  startConversation();
  await send(text);
  return true;
}
