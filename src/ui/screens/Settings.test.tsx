import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { defaultSettings } from '../../core/settings';
import { SettingsScreen } from './Settings';
import { AboutBuildDetails, AboutSettings } from './AboutSettings';
import { normalizeBuildInfo } from '../../core/build-info';

describe('SettingsScreen', () => {
  it('monta as seções de preferências', () => {
    const markup = renderToStaticMarkup(
      <SettingsScreen
        settings={defaultSettings}
        update={() => undefined}
        onOpenPermissions={() => undefined}
        performanceStatus={{
          enabled: false,
          reason: 'Automático desativado: perfil simulado acima do limite.',
          loading: false,
        }}
      />,
    );
    expect(markup).toContain('Provedores alternativos');
    expect(markup).toContain('Uso de tokens');
    expect(markup).toContain('Limite diário');
    expect(markup).toContain('Estado: Dentro do limite');
    expect(markup).toContain('Ao fechar a janela');
    expect(markup).toContain('Voltas máximas');
    expect(markup).toContain('Sempre perguntar');
    expect(markup).toContain('Abrir Permissões');
    expect(markup).not.toContain('Links externos');
    expect(markup).toContain('Preferências deste dispositivo');
    expect(markup).toContain('Reduzir mascote');
    expect(markup).toContain('A janela do pet continua seguindo Reduzir animações');
    expect(markup).toContain('Mostrar novidades de novas versões');
    expect(markup).toContain('Modo Desempenho');
    expect(markup).toContain('Automático desativado: perfil simulado acima do limite.');
  });

  it('exibe as informações de versão em um componente independente', () => {
    const markup = renderToStaticMarkup(
      <AboutSettings
        releaseNotesUrl="https://github.com/Anceryx/OdeKrai/blob/main/docs/releases/v0.15.0.md"
        onOpenReleaseNotes={() => undefined}
        settings={defaultSettings}
        update={() => undefined}
        onUpdateFound={() => undefined}
      />,
    );
    expect(markup).toContain('id="settings-about"');
    expect(markup).toContain('WebView2');
    expect(markup).toContain('Licença');
    expect(markup).toContain('Abrir pasta de logs');
    expect(markup).toContain('Notas completas da versão');
    expect(markup).toContain('Verificar atualizações automaticamente');
    expect(markup).toContain('Verificar agora');
  });

  it('normaliza hash e data de build ausentes antes de exibi-los', () => {
    const partialBuildInfo = normalizeBuildInfo({ version: '0.15.0' });
    expect(partialBuildInfo).toEqual({
      version: '0.15.0',
      commit: '',
      build_date: '',
      webview_version: '',
      windows_version: '',
    });
    expect(normalizeBuildInfo(undefined)).toBeNull();

    const details = renderToStaticMarkup(
      <AboutBuildDetails buildInfo={partialBuildInfo} onError={() => undefined} />,
    );
    expect(details).toContain('0.15.0');
    expect(details).toContain('Commit');
    expect(details).toContain('local');
    expect(details).toContain('Data do build (UTC)');
    expect(details).not.toContain('undefined');
  });
});
