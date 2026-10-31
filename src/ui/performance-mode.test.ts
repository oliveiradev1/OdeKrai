import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import islandWindow from './IslandWindow.tsx?raw';

const styles = readFileSync(new URL('./styles.css', import.meta.url), 'utf8');

describe('aplicação do modo Desempenho na interface', () => {
  it('remove sombras, filtros e gradientes, e deixa modais sem animação', () => {
    expect(styles).toContain("html[data-performance='on'] *");
    expect(styles).toContain('box-shadow: none !important');
    expect(styles).toContain('backdrop-filter: none !important');
    expect(styles).toContain('background-image: none !important');
    expect(styles).toMatch(
      /html\[data-performance='on'\] \.modal-surface\s*\{\s*animation: none !important/,
    );
    expect(styles).toContain("html[data-performance='on'] .pet-sheet");
    expect(styles).toContain("html[data-performance='on'] [data-translucent]");
  });

  it('mantém transparente o documento da janela do pet e recebe o estado calculado', () => {
    expect(styles).toMatch(
      /html\.island-window-html\[data-performance='on'\][\s\S]*?background: transparent !important/,
    );
    expect(styles).toContain("html[data-performance='on'] .island-shell");
    expect(islandWindow).toContain("listen<PerformanceModeStatus>('performance-mode-updated'");
    expect(islandWindow).toContain("dataset.performance = payload.enabled ? 'on' : 'off'");
  });
});
