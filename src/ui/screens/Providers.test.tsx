import { readFileSync } from 'node:fs';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { defaultProviderSettings, defaultSettings } from '../../core/settings';
import { AI_PROVIDERS } from '../../core/providers';
import { Icon } from '../Icon';
import { ProvidersScreen } from './Providers';
import { PROVIDER_TOOLS_HELP_TEXT, providerToolsBadgeLabel } from '../ProviderCatalog';

describe('ProvidersScreen', () => {
  it('usa um rótulo que indica suporte a ferramentas, sem sugerir conexão', () => {
    expect(
      providerToolsBadgeLabel({
        streaming: true,
        tools: true,
        vision: false,
        imageGeneration: false,
        verified: true,
      }),
    ).toBe('Suporta ferramentas');
    expect(PROVIDER_TOOLS_HELP_TEXT).toContain('não significa que o provedor esteja conectado');
  });

  it('monta o catálogo', () => {
    const markup = renderToStaticMarkup(
      <ProvidersScreen
        settings={defaultSettings}
        selectedId={AI_PROVIDERS[0].id}
        onSelect={() => undefined}
        onSetPrimary={() => undefined}
        onToggleEnabled={() => undefined}
        onSave={() => undefined}
        onChat={() => undefined}
        onCredentialRemoved={() => undefined}
      />,
    );
    expect(markup).toContain('Provedores de IA');
  });

  it('mostra as três abas e um ícone de fallback no cartão local', () => {
    const markup = renderToStaticMarkup(
      <ProvidersScreen
        settings={defaultSettings}
        selectedId="local"
        onSelect={() => undefined}
        onSetPrimary={() => undefined}
        onToggleEnabled={() => undefined}
        onSave={() => undefined}
        onChat={() => undefined}
        onCredentialRemoved={() => undefined}
      />,
    );

    expect(markup).toContain('Conectados');
    expect(markup).toContain('Gratuitos');
    expect(markup).toContain('Todos');
    expect(markup).toContain(renderToStaticMarkup(<Icon name="harddrives" size={18} />));
  });

  it('mantém estados dos cards legíveis sem credenciais e elimina alturas vazias forçadas', () => {
    const settings = {
      ...defaultSettings,
      providers: {
        openai: {
          ...defaultProviderSettings('openai'),
          lastTest: {
            status: 'failed' as const,
            at: '2026-10-29T00:00:00.000Z',
            message: 'Teste de conexão reprovado.',
          },
        },
      },
    };
    const markup = renderToStaticMarkup(
      <ProvidersScreen
        settings={settings}
        selectedId="openai"
        onSelect={() => undefined}
        onSetPrimary={() => undefined}
        onToggleEnabled={() => undefined}
        onSave={() => undefined}
        onChat={() => undefined}
        onCredentialRemoved={() => undefined}
      />,
    );
    const css = readFileSync(new URL('../styles.css', import.meta.url), 'utf8');

    expect(markup).toContain('Desconectado');
    expect(markup).toContain('Teste reprovado');
    expect(markup).toContain('Principal');
    expect(markup).toContain('Habilitado');
    expect(markup).toContain('Plano gratuito verificado');
    expect(markup).toContain('API local compatível com OpenAI');
    expect(markup).toContain('Local');
    expect(markup).toContain('Serviço local não verificado');
    expect(markup).not.toContain('Credencial salva');
    expect(css).toContain('grid-auto-rows: 1fr');
    expect(css).not.toContain('grid-auto-rows: minmax(236px, 1fr)');
    expect(css).not.toMatch(/\.provider-card\s*\{[^}]*min-height:\s*236px/);
    expect(css).toMatch(/\.provider-card \.provider-tile\s*\{[^}]*grid-template-rows:\s*auto auto/);
    expect(css).toMatch(/\.provider-badges span,[\s\S]*?overflow-wrap:\s*break-word/);
    expect(css).toMatch(/\.provider-tile-copy strong,[\s\S]*?word-break:\s*normal/);
    expect(css).toMatch(/\.provider-badges \.provider-badge--disconnected[\s\S]*?color:\s*#aaa7af/);
    expect(css).toMatch(/\.provider-badges \.provider-badge--local[\s\S]*?color:\s*#b9b9ff/);
  });
});
