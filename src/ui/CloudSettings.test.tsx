import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { CloudSettings, hostingerStatusAfterRequest, parseBudgetCents } from './CloudSettings';

describe('seção informativa de nuvem', () => {
  const markup = renderToStaticMarkup(<CloudSettings />);
  const savedMarkup = renderToStaticMarkup(<CloudSettings initialTokenSaved />);
  const connectedMarkup = renderToStaticMarkup(
    <CloudSettings initialTokenSaved initialConnected />,
  );

  it('distingue as capacidades de ShardCloud e Hostinger', () => {
    expect(markup).toContain('listar VMs, consultar detalhes e métricas e listar distribuições');
    expect(markup).toContain('iniciar ou parar uma VM');
    expect(markup).toContain('projetos Docker, firewalls, backups');
    expect(markup).toContain('firewalls e regras; chaves SSH públicas anexadas');
    expect(markup).toContain('criar ou atualizar projetos Docker');
    expect(markup).toContain('Sempre com aprovação nativa, em qualquer modo');
    expect(markup).toContain('excluir ou reinstalar VPS');
    expect(markup).toContain('restaurar snapshots');
    expect(markup).toContain('Criar ou comprar VPS não está disponível pelo OdeKrai.');
    expect(markup).toContain('Não conectado');
    expect(savedMarkup).toContain('ShardCloud');
    expect(savedMarkup).toContain('Token salvo');
  });

  it('apresenta as duas listas e marca todos os serviços apenas como planejados', () => {
    expect(markup).toContain('Nuvens brasileiras');
    expect(markup).toContain('Nuvens internacionais');
    expect(markup).toContain('Hospedagem com API');
    expect(markup).toContain('ShardCloud');
    expect(markup).toContain('Magalu Cloud');
    expect(markup).toContain('Locaweb');
    expect(markup).not.toContain('UOL Host');
    expect(markup).toContain('Amazon Web Services');
    expect(markup).toContain('Google Cloud');
    expect(markup).toContain('Microsoft Azure');
    expect(markup).toContain('Oracle Cloud');
    expect(markup.match(/Planejado/g)).toHaveLength(7);
    expect(markup).not.toContain('target="_blank"');
    expect(markup).not.toMatch(/compatível|disponível como integração/i);
  });

  it('não mostra de novo o token ShardCloud depois de salvo e oferece teste e remoção', () => {
    expect(markup).toContain('type="password"');
    expect(savedMarkup).not.toContain('id="shardcloud-token"');
    expect(savedMarkup).toContain('id="hostinger-token"');
    expect(savedMarkup).toContain('Testar conexão');
    expect(savedMarkup).toContain('Remover token');
    expect(markup).toContain('>Conectar</button>');
    const shardCloudCard = savedMarkup.slice(
      savedMarkup.indexOf('class="cloud-shardcloud"'),
      savedMarkup.indexOf('class="cloud-hostinger"'),
    );
    expect(shardCloudCard).not.toContain('Salvar token');
    expect(connectedMarkup).toContain('Conectado');
  });

  it('submete o token por formulário e não permite preenchimento automático', () => {
    const formStart = markup.indexOf('<form class="cloud-token-entry"');
    const formEnd = markup.indexOf('</form>', formStart);
    const formMarkup = markup.slice(formStart, formEnd);
    expect(formStart).toBeGreaterThanOrEqual(0);
    expect(formMarkup).toContain('id="shardcloud-token"');
    expect(formMarkup).toContain('name="shardcloud-token"');
    expect(formMarkup).toContain('type="password"');
    expect(formMarkup).toContain('autoComplete="off"');
    expect(formMarkup).toContain('type="submit"');
  });

  it('marca consultas externas como não confiáveis sem inserir respostas como HTML', () => {
    expect(connectedMarkup).toContain('Dados externos da ShardCloud — não confiáveis');
    expect(connectedMarkup).not.toContain('dangerouslySetInnerHTML');
  });

  it('usa links oficiais sem parâmetros de rastreamento e mantém o fluxo de abertura validado', () => {
    const hrefs = [...markup.matchAll(/href="([^"]+)"/g)].map((match) => match[1]);
    expect(hrefs).toHaveLength(9);
    for (const href of hrefs) {
      const url = new URL(href);
      expect(url.protocol).toBe('https:');
      expect(url.search).toBe('');
      expect(url.hash).toBe('');
      expect(url.href).not.toContain('utm_source=');
    }
    expect(markup).toContain('Documentação oficial da ShardCloud');
    expect(markup).toContain('Documentação oficial da Hostinger');
    expect(markup).toContain('>Conectar</button>');
  });
});

describe('teto local da ShardCloud', () => {
  it('exibe entrada do teto, estado sem configuração e o aviso de preço não verificado', () => {
    const markup = renderToStaticMarkup(<CloudSettings />);
    expect(markup).toContain('Teto mensal para VMs');
    expect(markup).toContain('id="shardcloud-monthly-budget"');
    expect(markup).toContain('Criar ou comprar VPS não está disponível pelo OdeKrai.');
    expect(markup).toContain('O preço real por VM não foi conferido.');
    expect(markup).toContain('ShardCloud não recebe nem aplica esse teto');
    expect(markup).toContain('O OdeKrai não cria VPS nem efetua pagamentos.');
  });

  it('converte valores brasileiros para centavos e recusa limites inválidos', () => {
    expect(parseBudgetCents('1,00')).toBe(100);
    expect(parseBudgetCents('125,5')).toBe(12_550);
    expect(parseBudgetCents('1000000')).toBe(100_000_000);
    expect(parseBudgetCents('0,99')).toBeNull();
    expect(parseBudgetCents('1000001')).toBeNull();
    expect(parseBudgetCents('abc')).toBeNull();
    expect(parseBudgetCents('1,999')).toBeNull();
  });
});

describe('cartão Hostinger', () => {
  it('usa os estados de conexão pedidos e não declara conexão sem teste', () => {
    const disconnected = renderToStaticMarkup(<CloudSettings />);
    const connected = renderToStaticMarkup(
      <CloudSettings
        initialHostingerTokenSaved
        initialHostingerStatus="Conectado"
        initialHostingerVms={[{ id: 12, hostname: 'vps.example', state: 'running', plan: 'KVM' }]}
      />,
    );
    const error = renderToStaticMarkup(
      <CloudSettings initialHostingerTokenSaved initialHostingerStatus="Erro" />,
    );

    expect(disconnected).toContain('Não conectado');
    expect(disconnected).not.toContain('Conectado');
    expect(connected).toContain('>Conectado</span>');
    expect(error).toContain('>Erro</span>');
    expect(hostingerStatusAfterRequest('Não conectado', false, true)).toBe('Não conectado');
    expect(hostingerStatusAfterRequest('Não conectado', true, true)).toBe('Conectado');
    expect(hostingerStatusAfterRequest('Não conectado', true, false)).toBe('Erro');
  });

  it('guarda token fora da UI após salvar e oferece somente remoção e teste', () => {
    const unsaved = renderToStaticMarkup(<CloudSettings />);
    const saved = renderToStaticMarkup(<CloudSettings initialHostingerTokenSaved />);

    expect(unsaved).toContain('id="hostinger-token"');
    expect(unsaved).toContain('name="hostinger-token"');
    expect(unsaved).toContain('autoComplete="off"');
    expect(saved).not.toContain('id="hostinger-token"');
    expect(saved).toContain('Token salvo');
    expect(saved).toContain('Testar conexão');
    expect(saved).toContain('Remover token');
    expect(unsaved).toContain('>Conectar</button>');
  });

  it('explica a criação, o alcance e a expiração curta do token', () => {
    const markup = renderToStaticMarkup(<CloudSettings />);
    expect(markup).toContain('Informações da conta → API → Novo token');
    expect(markup).toContain('acesso à conta inteira, inclusive ao faturamento');
    expect(markup).toContain('escolha uma expiração curta');
  });

  it('renderiza cada status da ShardCloud', () => {
    const disconnected = renderToStaticMarkup(<CloudSettings />);
    const connected = renderToStaticMarkup(
      <CloudSettings initialTokenSaved initialConnected initialShardcloudStatus="Conectado" />,
    );
    const error = renderToStaticMarkup(
      <CloudSettings initialTokenSaved initialShardcloudStatus="Erro" />,
    );
    expect(disconnected).toContain('>Não conectado</span>');
    expect(connected).toContain('>Conectado</span>');
    expect(error).toContain('>Erro</span>');
  });

  it('exibe as consultas disponíveis e rotula as respostas como dados externos', () => {
    const connected = renderToStaticMarkup(
      <CloudSettings
        initialHostingerTokenSaved
        initialHostingerStatus="Conectado"
        initialHostingerVms={[{ id: 12, hostname: 'vps.example', state: 'running', plan: 'KVM' }]}
      />,
    );
    for (const action of [
      'Atualizar máquinas',
      'Detalhes',
      'Ações recentes',
      'Backups',
      'Data centers',
      'Templates de sistema',
    ]) {
      expect(connected).toContain(action);
    }
    expect(connected).toContain('Dados externos da Hostinger — não confiáveis');
    expect(connected).toContain('Identificador: 12');
    expect(connected).not.toContain('dangerouslySetInnerHTML');
    expect(connected).toContain(
      'As ações de iniciar, parar ou reiniciar pelo agente sempre pedem sua aprovação.',
    );
  });
});

describe('ações pendentes na Nuvem', () => {
  it('mostra o pedido aguardando aprovação e abre a revisão nativa sem aprovar no painel', () => {
    const markup = renderToStaticMarkup(
      <CloudSettings
        initialPendingCloudAction={{
          id: 'pending-1',
          tool: 'shardcloud.vm_start',
          origin: 'Ação proposta pelo modelo',
          summary: 'Iniciar a VM web-1',
          preview: 'VM: web-1 (vm-123)\nAção: iniciar',
          fullPreview: false,
        }}
      />,
    );
    expect(markup).toContain('Ações pendentes');
    expect(markup).toContain('Iniciar a VM web-1');
    expect(markup).toContain('VM: web-1 (vm-123)');
    expect(markup).toContain('Revisar na janela de aprovação');
    expect(markup).not.toMatch(/Aprovar|Negar|Recusar/);
  });

  it('mostra um estado vazio quando não há ação de nuvem pendente', () => {
    const markup = renderToStaticMarkup(<CloudSettings />);
    expect(markup).toContain('Nenhuma ação de nuvem aguarda aprovação.');
  });
});
