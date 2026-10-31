import { describe, expect, it } from 'vitest';
import settingsSource from '../ui/screens/Settings.tsx?raw';
import aboutSettingsSource from '../ui/screens/AboutSettings.tsx?raw';
import agentHomeSource from '../ui/AgentHome.tsx?raw';
import assistantSource from '../ui/screens/Assistant.tsx?raw';
import computerSource from '../ui/screens/Computer.tsx?raw';
import settingsModalSource from '../ui/SettingsModal.tsx?raw';
import appNavigationSource from '../ui/hooks/useNavigation.ts?raw';
import securityCenterSource from '../ui/SecurityCenter.tsx?raw';
import privacyConfigurationsSource from '../ui/PrivacyConfigurations.tsx?raw';
import discordSettingsSource from '../ui/DiscordWebhooksSettings.tsx?raw';
import cloudSettingsSource from '../ui/CloudSettings.tsx?raw';
import {
  filterSettingsIndex,
  filterSettingsMenu,
  SETTINGS_INDEX,
  SETTINGS_MENU,
  settingsMenuLayout,
} from './settings-navigation';

const sourceBySection = {
  geral: settingsSource,
  'perfil-memoria': agentHomeSource,
  assistente: assistantSource,
  computador: computerSource,
  habilidades: settingsModalSource,
  conectores: settingsModalSource,
  privacidade: `${securityCenterSource}\n${privacyConfigurationsSource}\n${discordSettingsSource}`,
  atividade: securityCenterSource,
  nuvem: cloudSettingsSource,
  sobre: aboutSettingsSource,
} as const;

describe('navegação das configurações', () => {
  it('lista as dez seções previstas, com Nuvem logo após Atividade', () => {
    expect(SETTINGS_MENU.map((section) => section.id)).toEqual([
      'geral',
      'perfil-memoria',
      'assistente',
      'computador',
      'habilidades',
      'conectores',
      'privacidade',
      'atividade',
      'nuvem',
      'sobre',
    ]);
    expect(filterSettingsMenu('nuvem').map((section) => section.id)).toContain('nuvem');
    expect(filterSettingsIndex('nuvem')).toContainEqual(
      expect.objectContaining({ section: 'nuvem', anchor: 'cloud-shardcloud-heading' }),
    );
  });

  it('filtra opções e seções pelo mesmo índice', () => {
    expect(filterSettingsIndex('desempenho').map((entry) => entry.anchor)).toContain(
      'settings-performance',
    );
    expect(filterSettingsIndex('novidades').map((entry) => entry.anchor)).toContain(
      'settings-updates',
    );
    expect(filterSettingsIndex('microfone').map((entry) => entry.anchor)).toContain(
      'settings-audio',
    );
    expect(filterSettingsMenu('microfone').map((section) => section.id)).toContain('geral');
    expect(filterSettingsIndex('memoria').map((entry) => entry.anchor)).toContain(
      'identity-memory',
    );
    expect(filterSettingsMenu('privacidade').map((section) => section.id)).toContain('privacidade');
    expect(
      filterSettingsIndex('habilidades').some((entry) => entry.section === 'habilidades'),
    ).toBe(true);
    expect(
      filterSettingsIndex('provedor principal').some((entry) => entry.section === 'conectores'),
    ).toBe(true);
  });

  it('mapeia cada seção de configurações para um componente e evita âncoras duplicadas', () => {
    for (const section of SETTINGS_MENU) {
      expect(settingsModalSource).toContain(`case '${section.id}'`);
    }
    expect(new Set(SETTINGS_INDEX.map((entry) => entry.anchor)).size).toBe(SETTINGS_INDEX.length);
    expect(settingsModalSource).not.toContain('Preferências globais');
    expect(settingsModalSource).toContain("onOpenPage={() => onOpenPage('skills')}");
    expect(settingsModalSource).toContain("onOpenPage={() => onOpenPage('providers')}");
    expect(appNavigationSource).toContain(
      'const nextView: NavigationView = { ...next, settingsOpen: false }',
    );
  });

  it('garante que cada opção do índice aponta para um ID existente na tela correspondente', () => {
    const sources = new Map(Object.entries(sourceBySection));
    for (const entry of SETTINGS_INDEX) {
      expect(sources.get(entry.section), entry.title).toContain(entry.anchor);
    }
  });

  it('usa lista suspensa abaixo de 800 px e menu vertical a partir de 800 px', () => {
    expect(settingsMenuLayout(799)).toBe('select');
    expect(settingsMenuLayout(800)).toBe('vertical');
  });
});
