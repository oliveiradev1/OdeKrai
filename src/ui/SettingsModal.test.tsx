import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { defaultProviderSettings, defaultSettings } from '../core/settings';
import {
  ConnectorsSettingsSummary,
  SkillsSettingsSummary,
  summarizeConnectors,
  summarizeSkillsStatus,
} from './SettingsModal';

describe('resumos das seções do modal Configurações', () => {
  it('resume habilidades ativas a partir dos estados registrados pelo Rust', () => {
    expect(
      summarizeSkillsStatus([
        { status: 'installed' },
        { status: 'available' },
        { status: 'configuration_required' },
        { status: 'unavailable' },
      ]),
    ).toEqual({ total: 4, active: 1 });
  });

  it('mostra o estado do catálogo e encaminha à central sem repetir controles', () => {
    const markup = renderToStaticMarkup(<SkillsSettingsSummary onOpenPage={() => undefined} />);
    expect(markup).toContain('id="skills-summary"');
    expect(markup).toContain('Abrir central de habilidades');
    expect(markup).not.toContain('Reduzir animações');
    expect(markup).not.toContain('Usar mascote nas respostas');
  });

  it('resume provedor principal, modelo e reservas sem afirmar conexão por credenciais', () => {
    const settings = {
      ...defaultSettings,
      activeProviderId: 'openai',
      fallbackProviderIds: ['anthropic', 'local'],
      providers: {
        openai: { ...defaultProviderSettings('openai'), model: 'gpt-5.1' },
        anthropic: { ...defaultProviderSettings('anthropic'), model: 'claude-test' },
        local: { ...defaultProviderSettings('local'), enabled: false, model: 'local-test' },
      },
    };
    expect(summarizeConnectors(settings)).toEqual({
      providerName: 'OpenAI',
      model: 'gpt-5.1',
      enabled: true,
      fallbackCount: 1,
    });

    const markup = renderToStaticMarkup(
      <ConnectorsSettingsSummary settings={settings} onOpenPage={() => undefined} />,
    );
    expect(markup).toContain('id="connectors-summary"');
    expect(markup).toContain('OpenAI · habilitado · modelo gpt-5.1');
    expect(markup).toContain('Abrir central de conectores');
    expect(markup).not.toContain('Conectado');
    expect(markup).not.toContain('Reduzir animações');
    expect(markup).not.toContain('Usar mascote nas respostas');
  });

  it('tolera configurações persistidas sem model e resume modelo vazio', () => {
    const legacySettings = {
      ...defaultSettings,
      activeProviderId: 'openai',
      fallbackProviderIds: ['anthropic'],
      providers: {
        openai: { ...defaultProviderSettings('openai'), model: undefined },
        anthropic: { ...defaultProviderSettings('anthropic'), model: undefined },
      },
    } as unknown as typeof defaultSettings;

    expect(summarizeConnectors(legacySettings)).toEqual({
      providerName: 'OpenAI',
      model: null,
      enabled: true,
      fallbackCount: 0,
    });
    expect(
      renderToStaticMarkup(
        <ConnectorsSettingsSummary settings={legacySettings} onOpenPage={() => undefined} />,
      ),
    ).toContain('OpenAI');
  });
});
