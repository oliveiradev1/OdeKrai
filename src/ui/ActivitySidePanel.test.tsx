import { renderToStaticMarkup } from 'react-dom/server';
import type { ComponentProps } from 'react';
import { describe, expect, it, vi } from 'vitest';
import {
  ActivityPanelTrigger,
  ActivitySidePanel,
  activityPanelTabIndex,
  focusActivityPanel,
  shouldLoadPanelAudit,
  toggleActivityPanel,
} from './ActivitySidePanel';
import type { AgentActionCardData } from './AgentControls';

const action: AgentActionCardData = {
  id: 'action-1',
  skillId: 'files.write',
  title: 'Salvar relatório',
  risk: 'escrita reversível',
  decision: 'automática',
  status: 'Executando',
  summary: 'Gravando o relatório.',
  timeline: [{ label: 'Preparar arquivo', status: 'em andamento', at: 1 }],
  artifacts: [
    { path: 'C:\\workspace\\relatorio.txt', kind: 'documento' },
    { path: 'C:\\workspace\\programa.exe', kind: 'arquivo' },
  ],
};

function panel(overrides: Partial<ComponentProps<typeof ActivitySidePanel>> = {}) {
  return renderToStaticMarkup(
    <ActivitySidePanel
      pendingApprovalCount={2}
      running
      awaitingApproval={false}
      currentAction={action}
      events={[{ id: 'event-1', title: 'Reunião', start: '2026-10-31T15:00:00.000Z' }]}
      reminderCount={1}
      reminders={[
        { title: 'Ligar para Ana', whenLocal: '2026-10-30T15:30:00-03:00', status: 'pending' },
      ]}
      remindersLoading={false}
      upcomingReminderUnavailable={false}
      scheduleUnavailable={false}
      reminderUnavailable={false}
      actions={[action]}
      artifactHistoryAvailable
      audit={[
        {
          id: 'audit-1',
          timestamp: 1793458800,
          tool: 'files.write',
          status: 'completed',
          decision: 'allow',
        },
      ]}
      scheduleLoading={false}
      auditLoading={false}
      auditUnavailable={false}
      triggerRef={{ current: null }}
      onClose={() => undefined}
      onStop={() => undefined}
      onReview={() => undefined}
      onOpenArtifact={() => undefined}
      onRevealArtifact={() => undefined}
      onViewAll={() => undefined}
      {...overrides}
    />,
  );
}

describe('painel lateral de atividade', () => {
  it('mostra as seções com dados reais, resumo sem parâmetros e ações seguras', () => {
    const markup = panel();
    for (const section of [
      'Em execução',
      'Aguardando você',
      'Agendado',
      'Arquivos recentes',
      'Histórico resumido',
    ])
      expect(markup).toContain(section);
    expect(markup).toContain('Execução: Local');
    expect(markup).toContain('Reunião');
    expect(markup).toContain('lembrete pendente');
    expect(markup).toContain('Ligar para Ana');
    expect(markup).toContain('15:30');
    expect(markup).toContain('Preparar arquivo');
    expect(markup).toContain('C:\\workspace\\relatorio.txt');
    expect(markup).not.toContain('programa.exe');
    expect(markup).toContain('Salvar arquivo');
    expect(markup).toContain('Concluída');
    expect(markup).not.toContain('paramsSummary');
    expect(markup).toContain('Ver tudo');
    expect(markup).toContain('Parar');
    expect(markup).toContain('Revisar');
  });

  it('apresenta estados vazios honestos e não oferece decisão de aprovação', () => {
    const markup = panel({
      pendingApprovalCount: 0,
      running: false,
      currentAction: null,
      events: [],
      reminderCount: 0,
      reminders: [],
      remindersLoading: false,
      actions: [],
      audit: [],
    });
    expect(markup).toContain('Nenhuma execução em andamento');
    expect(markup).toContain('Nenhuma aprovação pendente');
    expect(markup).toContain('Nenhum evento nas próximas 24 horas');
    expect(markup).toContain('Nenhum lembrete nas próximas 24 horas');
    expect(markup).toContain('Nenhum arquivo recente nesta conversa');
    expect(markup).toContain('Nenhuma ação recente registrada');
    expect(markup).not.toMatch(/aprovar|negar/i);
  });

  it('não renderiza o painel fechado e mantém o contador acessível no gatilho', () => {
    expect(panel({ open: false })).toBe('');
    const trigger = renderToStaticMarkup(
      <ActivityPanelTrigger
        count={3}
        expanded={false}
        buttonRef={{ current: null }}
        onClick={() => undefined}
      />,
    );
    expect(trigger).toContain('aria-expanded="false"');
    expect(trigger).toContain('3');
    expect(toggleActivityPanel(false)).toBe(true);
    expect(toggleActivityPanel(true)).toBe(false);
  });

  it('não consulta histórico enquanto fechado e devolve o foco ao fechar', () => {
    expect(shouldLoadPanelAudit(false)).toBe(false);
    expect(shouldLoadPanelAudit(true)).toBe(true);
    const trigger = { focus: vi.fn() };
    const closeButton = { focus: vi.fn() };
    const cleanup = focusActivityPanel(trigger, closeButton);
    expect(closeButton.focus).toHaveBeenCalledOnce();
    cleanup();
    expect(trigger.focus).toHaveBeenCalledOnce();
  });

  it('trata Escape e mantém Tab e Shift+Tab dentro do painel modal', () => {
    expect(activityPanelTabIndex('Escape', false, 0, 3)).toBe(-1);
    expect(activityPanelTabIndex('Tab', false, 2, 3)).toBe(0);
    expect(activityPanelTabIndex('Tab', true, 0, 3)).toBe(2);
    expect(activityPanelTabIndex('Tab', false, 1, 3)).toBeNull();
  });
});
