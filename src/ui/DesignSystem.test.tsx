import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import {
  Badge,
  Banner,
  Button,
  Card,
  EmptyState,
  getTabsKeyboardIndex,
  Kbd,
  ListItem,
  PageHeader,
  SelectableCard,
  SelectInput,
  Skeleton,
  StatusBadge,
  Tabs,
  TextInput,
  Toggle,
  Toast,
  Tooltip,
} from './DesignSystem';
import { DesignSystemGallery } from './DesignSystemGallery';

describe('componentes visuais compartilhados', () => {
  it('aplica a variante solicitada ao botão', () => {
    expect(renderToStaticMarkup(<Button variant="primary">Salvar</Button>)).toContain(
      'ui-button--primary',
    );
  });

  it('mantém estado e nome acessíveis em alternância e cartão selecionável', () => {
    const markup = renderToStaticMarkup(
      <>
        <Toggle label="Ativar sons" checked />
        <SelectableCard selected onClick={() => undefined}>
          Sempre perguntar
        </SelectableCard>
      </>,
    );
    expect(markup).toContain('aria-label="Ativar sons"');
    expect(markup).toContain('aria-checked="true"');
    expect(markup).toContain('aria-pressed="true"');
  });

  it('aplica papel, tom e estrutura comuns de aviso, selo e cabeçalho', () => {
    const markup = renderToStaticMarkup(
      <>
        <Banner tone="warning">Confirme a alteração.</Banner>
        <Badge tone="success">Ativo</Badge>
        <PageHeader eyebrow="PERFIL" title="Seu perfil" subtitle="Dados locais." />
      </>,
    );
    expect(markup).toContain('role="status"');
    expect(markup).toContain('ui-banner--warning');
    expect(markup).toContain('ui-badge--success');
    expect(markup).toContain('<h1>Seu perfil</h1>');
  });

  it('oferece cartão, campo de texto e seletor com classes compartilhadas', () => {
    const markup = renderToStaticMarkup(
      <>
        <Card aria-label="Preferências" />
        <TextInput aria-label="Nome" />
        <SelectInput aria-label="Idioma">
          <option>Português</option>
        </SelectInput>
      </>,
    );
    expect(markup).toContain('ui-card');
    expect(markup).toContain('ui-input');
    expect(markup).toContain('ui-select');
  });

  it('renderiza e nomeia os estados, abas, avisos e estado vazio', () => {
    const markup = renderToStaticMarkup(
      <>
        {(['ativa', 'desativada', 'pendente', 'erro', 'sucesso'] as const).map((state) => (
          <StatusBadge key={state} state={state} />
        ))}
        <Tabs
          label="Preferências"
          selectedId="geral"
          onSelect={() => undefined}
          items={[
            { id: 'geral', label: 'Geral', content: 'Geral' },
            { id: 'avancado', label: 'Avançado', content: 'Avançado' },
          ]}
        />
        <Toast action={{ onClick: () => undefined }}>Alteração salva</Toast>
        <EmptyState
          icon={<span>◎</span>}
          title="Sem resultados"
          description="Ajuste a pesquisa."
          action={{ label: 'Limpar', onClick: () => undefined }}
        />
      </>,
    );
    expect(markup).toContain('role="tablist" aria-label="Preferências"');
    expect(markup).toContain('role="tabpanel"');
    expect(markup).toContain('aria-live="polite"');
    expect(markup).toContain('Desfazer');
    expect(markup).toContain('aria-labelledby=');
    expect(markup).toContain('Limpar');
    for (const state of ['ativa', 'desativada', 'pendente', 'erro', 'sucesso']) {
      expect(markup).toContain(`ui-status-badge--${state}`);
    }
  });

  it('cobre navegação de abas por teclado, dica associada e semântica', () => {
    const tabs = [{ disabled: false }, { disabled: true }, { disabled: false }];
    expect(getTabsKeyboardIndex('ArrowRight', 0, tabs)).toBe(2);
    expect(getTabsKeyboardIndex('ArrowLeft', 2, tabs)).toBe(0);
    expect(getTabsKeyboardIndex('Home', 2, tabs)).toBe(0);
    expect(getTabsKeyboardIndex('End', 0, tabs)).toBe(2);
    expect(getTabsKeyboardIndex('Home', 0, [{ disabled: true }])).toBeNull();
    expect(getTabsKeyboardIndex('Escape', 0, tabs)).toBeNull();

    const markup = renderToStaticMarkup(
      <>
        <Tooltip content="Explicação">
          <button type="button">Ajuda</button>
        </Tooltip>
        <Kbd>Ctrl</Kbd>
        <Skeleton />
        <ul>
          <ListItem description="Detalhe" trailing="Pronto">
            Nome
          </ListItem>
        </ul>
      </>,
    );
    expect(markup).toContain('role="tooltip"');
    expect(markup).toContain('aria-describedby=');
    expect(markup).toContain('<kbd');
    expect(markup).toContain('aria-hidden="true"');
    expect(markup).toContain('<li');
    expect(markup).toContain('ui-list-item-description');
  });

  it('usa a galeria interna como amostra integrada dos componentes', () => {
    const markup = renderToStaticMarkup(<DesignSystemGallery />);
    expect(markup).toContain('Galeria do sistema visual');
    expect(markup).toContain('Desfazer');
    expect(markup).toContain('role="tablist"');
    expect(markup).toContain('ui-status-badge--pendente');
    expect(markup).toContain('ui-empty-state');
  });
});
