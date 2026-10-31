import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import { AgentActionCard, AgentControls, type AgentActionCardData } from './AgentControls';

const action = (status: string, patch: Partial<AgentActionCardData> = {}): AgentActionCardData => ({
  id: `card-${status.replaceAll(' ', '-')}`,
  skillId: 'workspace.write_file',
  title: 'Salvar arquivo',
  risk: 'escrita reversível',
  decision: 'aguardando',
  status,
  summary: 'Salvar arquivo local.',
  ...patch,
});

describe('controles e cartões do agente', () => {
  it('mostra estado indisponível acessível e interruptor desativado', () => {
    const markup = renderToStaticMarkup(
      <AgentControls
        enabled={false}
        disabledReason="Configure a pasta de trabalho."
        workspacePath={null}
        running={false}
        awaitingApproval={false}
        onToggle={() => undefined}
        onStop={() => undefined}
        onConfigure={() => undefined}
      />,
    );
    expect(markup).toContain('role="switch"');
    expect(markup).toContain('aria-checked="false"');
    expect(markup).toContain('Configure a pasta de trabalho.');
  });

  it.each([
    ['planejada', 'planejada'],
    ['Aguardando aprovação', 'aguardando aprovação'],
    ['Executando', 'executando'],
    ['Configuração pendente', 'configuração pendente'],
    ['Concluída', 'concluída'],
    ['Negada', 'negada'],
    ['Expirada', 'expirada'],
    ['Falhou', 'erro'],
  ])('renderiza estado %s', (status, expected) => {
    expect(renderToStaticMarkup(<AgentActionCard action={action(status)} />)).toContain(
      `Estado: ${expected}`,
    );
  });

  it('explica que falta cadastrar webhook e oferece configuração sem chamar de negação local', () => {
    const markup = renderToStaticMarkup(
      <AgentActionCard
        action={action('Configuração pendente', {
          skillId: 'discord.send',
          summary: 'Falta cadastrar um webhook do Discord.',
          resultSummary: 'Falta cadastrar um webhook do Discord.',
        })}
        onConfigureDiscord={() => undefined}
      />,
    );
    expect(markup).toContain('Estado: configuração pendente');
    expect(markup).toContain('Falta cadastrar um webhook do Discord.');
    expect(markup).toContain('Cadastrar webhook');
    expect(markup).not.toContain('bloqueada pelas regras locais');
  });

  it('não oferece botões de decisão e encaminha revisão para janela nativa', () => {
    const onReview = vi.fn();
    const markup = renderToStaticMarkup(
      <AgentActionCard
        action={action('Aguardando aprovação', {
          approvalExpiresAt: Math.floor(Date.now() / 1000) + 60,
        })}
        onReviewApproval={onReview}
      />,
    );
    expect(markup).toContain('Revisar na janela de aprovação');
    expect(markup).not.toMatch(/aprovar|negar/i);
    expect(onReview).not.toHaveBeenCalled();
  });

  it('só mostra desfazer quando o resultado tem descritor reversível', () => {
    const descriptor = {
      skillId: 'theme.restore',
      stateId: 'op-123',
      label: 'Restaurar tema anterior',
    };
    const reversible = renderToStaticMarkup(
      <AgentActionCard
        action={action('Concluída', { undoDescriptor: descriptor })}
        onUndo={() => undefined}
      />,
    );
    const irreversible = renderToStaticMarkup(
      <AgentActionCard
        action={action('Concluída', { skillId: 'discord.send' })}
        onUndo={() => undefined}
      />,
    );
    expect(reversible).toContain('Desfazer');
    expect(irreversible).not.toContain('Desfazer');
  });

  it('mantém falhas ao desfazer dentro do cartão que originou a ação', () => {
    const markup = renderToStaticMarkup(
      <AgentActionCard
        action={action('Concluída', {
          undoError: 'Parâmetros inválidos para restaurar o arquivo.',
        })}
      />,
    );
    expect(markup).toContain('role="alert"');
    expect(markup).toContain(
      'Não foi possível desfazer: Parâmetros inválidos para restaurar o arquivo.',
    );
    expect(markup).not.toContain('diagnostic-banner');
    expect(markup).not.toContain('class="toast"');
  });

  it('renderiza prévia como texto, detalhes, timeline e dado externo não confiável', () => {
    const markup = renderToStaticMarkup(
      <AgentActionCard
        action={action('Concluída', {
          preview: '<script>alert(1)</script>\n- arquivo.txt',
          resultSummary: 'Resultado externo.',
          untrustedResult: true,
          durationMs: 1250,
          timeline: [{ label: 'Gravar arquivo', status: 'concluída', durationMs: 1250, at: 1 }],
        })}
        running
        onStop={() => undefined}
      />,
    );
    expect(markup).toContain('&lt;script&gt;');
    expect(markup).toContain('dado não confiável');
    expect(markup).toContain('Gravar arquivo');
    expect(markup).toContain('Parar');
    expect(markup).toContain('1.3 s');
  });

  it('exibe plano informativo numerado, estados reais e ações fora do plano como texto', () => {
    const markup = renderToStaticMarkup(
      <AgentActionCard
        action={action('Concluída', {
          plan: {
            title: 'Preparar relatório',
            steps: [
              { description: 'Ler dados', status: 'completed' },
              { description: '<script>não executar</script>', status: 'in_progress' },
            ],
            outsidePlanActions: ['Atualizar agenda'],
          },
        })}
      />,
    );
    expect(markup).toContain('aria-label="Etapas do plano"');
    expect(markup).toContain('Ler dados');
    expect(markup).toContain('Concluída');
    expect(markup).toContain('&lt;script&gt;não executar&lt;/script&gt;');
    expect(markup).toContain('Ação fora do plano');
    expect(markup).toContain('Atualizar agenda');
    expect(markup).toContain('apenas informativo');
  });

  it('expõe controles de artefato somente para caminho de arquivo seguro', () => {
    const markup = renderToStaticMarkup(
      <AgentActionCard
        action={action('Concluída', {
          artifacts: [
            { path: 'C:\\workspace\\relatorio.txt', kind: 'documento' },
            { path: 'C:\\workspace\\payload.exe', kind: 'arquivo' },
            { path: '../fora.txt', kind: 'arquivo' },
          ],
        })}
        onOpenArtifact={() => undefined}
        onRevealArtifact={() => undefined}
      />,
    );
    expect(markup).toContain('Abrir');
    expect(markup).toContain('Mostrar na pasta');
    expect(markup).not.toContain('payload.exe');
    expect(markup).not.toContain('fora.txt');
  });
});
