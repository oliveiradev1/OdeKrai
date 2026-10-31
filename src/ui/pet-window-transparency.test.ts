import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

const styles = readFileSync(new URL('./styles.css', import.meta.url), 'utf8');
const islandWindow = readFileSync(new URL('./IslandWindow.tsx', import.meta.url), 'utf8');
const nativeWindow = readFileSync(new URL('../../src-tauri/src/lib.rs', import.meta.url), 'utf8');

describe('transparência da janela do pet', () => {
  it('mantém html, body e root transparentes, inclusive no modo Desempenho', () => {
    expect(styles).toMatch(
      /body\.island-window-body #root\s*\{[^}]*background: transparent !important/s,
    );
    expect(styles).toMatch(
      /html\.island-window-html\s*,\s*html\.island-window-html body\.island-window-body\s*\{[^}]*background: transparent !important/s,
    );
    expect(styles).toMatch(
      /html\.island-window-html\[data-performance='on'\][\s\S]*?#root\s*\{[^}]*background: transparent !important/s,
    );
  });

  it('remove a caixa do modo Livre mesmo com Desempenho ligado e preserva a sombra oval do sprite', () => {
    expect(styles).toMatch(
      /\.free-mode \.island-shell\s*\{[^}]*background: transparent;[^}]*box-shadow: none;/s,
    );
    expect(styles).toMatch(
      /html\[data-performance='on'\] \.free-mode \.island-shell\s*\{[^}]*background: transparent !important;[^}]*box-shadow: none !important;/s,
    );
    expect(styles).toMatch(
      /\.pet-shadow\s*\{[^}]*border-radius: 50%;[^}]*background: #0009;[^}]*filter: blur\(4px\);/s,
    );
    expect(styles).toMatch(
      /html\[data-performance='on'\] \.free-mode \.pet-shadow\s*\{[^}]*filter: blur\(4px\) !important;/s,
    );
  });

  it('preserva a superfície visual da Ilha e não altera a área nativa de clique', () => {
    expect(styles).toMatch(
      /\.island-shell\s*\{[^}]*background:[\s\S]*?box-shadow: 0 6px 22px #0008;/,
    );
    expect(islandWindow).toContain("settings.companion.freeMode ? 'free-mode' : 'island-mode'");
    expect(islandWindow).toContain('data-tauri-drag-region={settings.companion.freeMode ?');
    expect(nativeWindow).toMatch(
      /\.decorations\(false\)[\s\S]*?\.transparent\(true\)[\s\S]*?\.shadow\(false\)/,
    );
    expect(nativeWindow).not.toMatch(
      /ignore_cursor_events|set_ignore_cursor|setIgnoreCursorEvents/i,
    );
  });
});
