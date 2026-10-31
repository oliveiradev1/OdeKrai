import { describe, expect, it, vi } from 'vitest';
import { SETTINGS_INDEX, SETTINGS_MENU } from './settings-navigation';
import { skillFamilyPresentation } from './skills-catalog';
import { PETS } from './pets';
import {
  buildPaletteItems,
  COMMAND_PALETTE_ITEMS,
  dispatchPaletteAction,
  isValidPaletteDestination,
  nextPaletteIndex,
  searchPaletteItems,
  updateRecentCommands,
  type PaletteActionHandlers,
} from './command-palette';
import paletteSource from '../ui/CommandPalette.tsx?raw';
import aboutSource from '../ui/screens/AboutSettings.tsx?raw';
import appSource from '../ui/App.tsx?raw';
import permissionRustSource from '../../src-tauri/src/security/mod.rs?raw';

describe('paleta de comandos', () => {
  const items = buildPaletteItems({
    conversations: [{ id: 'abc', title: 'Planejar semana' }],
    currentConversationId: 'abc',
    agentEnabled: false,
  });

  it('combina busca aproximada, acentos e itens recentes', () => {
    expect(
      searchPaletteItems(items, 'convesa').some((item) => item.id === 'conversation:abc'),
    ).toBe(true);
    expect(searchPaletteItems(items, 'privacidade')[0].id).toContain('settings:privacidade');
    expect(searchPaletteItems(items, '', ['conversation:abc'])[0].id).toBe('conversation:abc');
    expect(updateRecentCommands(['a', 'b'], 'b')).toEqual(['b', 'a']);
  });

  it('faz cada destino derivar das rotas, índice, famílias e mascotes reais', () => {
    expect(items.every(isValidPaletteDestination)).toBe(true);
    expect(
      COMMAND_PALETTE_ITEMS.filter((item) => item.id.startsWith('settings:')).map(
        (item) => item.id,
      ),
    ).toEqual(SETTINGS_MENU.map(({ id }) => `settings:${id}`));
    expect(items.filter((item) => item.id.startsWith('setting:'))).toHaveLength(
      SETTINGS_INDEX.length,
    );
    expect(items.filter((item) => item.id.startsWith('skill:'))).toHaveLength(
      Object.keys(skillFamilyPresentation).length,
    );
    expect(
      items.filter((item) => item.id.startsWith('pet:') && item.id !== 'pet:mute'),
    ).toHaveLength(PETS.length);
  });

  it('despacha cada tipo de ação para seu destino correspondente', () => {
    const spy = vi.fn();
    const handlers: PaletteActionHandlers = Object.fromEntries(
      [
        'page',
        'settings',
        'newConversation',
        'conversation',
        'agentMode',
        'skillFamily',
        'pet',
        'permissionMode',
        'stopAll',
        'mute',
      ].map((key) => [key, spy]),
    ) as unknown as PaletteActionHandlers;
    const actions = new Set(items.map((item) => item.action.type));
    expect(actions).toEqual(
      new Set([
        'page',
        'settings',
        'new-conversation',
        'conversation',
        'agent-mode',
        'skill-family',
        'pet',
        'permission-mode',
        'stop-all',
        'mute',
      ]),
    );
    for (const item of items) dispatchPaletteAction(item.action, handlers);
    expect(spy).toHaveBeenCalledTimes(items.length);
  });

  it('mantém modo de permissão no comando nativo com confirmação e mostra o atalho no Sobre', () => {
    expect(appSource).toContain("invoke('set_permission_mode', { mode })");
    expect(permissionRustSource).toMatch(
      /pub async fn set_permission_mode[\s\S]*?confirm_native[\s\S]*?set_db_setting/,
    );
    expect(paletteSource).toContain('aria-activedescendant');
    expect(paletteSource).toContain('role="combobox"');
    expect(paletteSource).toContain('role="listbox"');
    expect(paletteSource).toContain('role="option"');
    expect(aboutSource).toContain('Ctrl+K');
    expect(appSource).toContain('setCommandPaletteOpen(false)');
  });

  it('navega com setas e envolve o início e o fim da lista', () => {
    expect(nextPaletteIndex('ArrowDown', 0, 3)).toBe(1);
    expect(nextPaletteIndex('ArrowUp', 0, 3)).toBe(2);
    expect(nextPaletteIndex('Enter', 0, 3)).toBeNull();
    expect(nextPaletteIndex('ArrowDown', 0, 0)).toBeNull();
  });
});
