import { describe, expect, it } from 'vitest';
import islandWindowSource from '../ui/IslandWindow.tsx?raw';
import { petHitTestPayload } from './pet-hit-test';

describe('payload de hit-test da janela do pet', () => {
  it('envia o retângulo visível do sprite quando o modo Livre está recolhido', () => {
    expect(
      petHitTestPayload(true, false, {
        left: 24,
        top: 36,
        width: 88,
        height: 92,
      }),
    ).toEqual({ enabled: true, x: 24, y: 36, width: 88, height: 92 });
  });

  it('desativa o hit-test quando o pet está expandido ou o modo Livre está desativado', () => {
    const bounds = { left: 24, top: 36, width: 88, height: 92 };
    expect(petHitTestPayload(true, true, bounds).enabled).toBe(false);
    expect(petHitTestPayload(false, false, bounds).enabled).toBe(false);
  });

  it('sem retângulo mantém a configuração desativada e envia dimensões seguras', () => {
    expect(petHitTestPayload(true, false, null)).toEqual({
      enabled: false,
      x: 0,
      y: 0,
      width: 0,
      height: 0,
    });
  });

  it('IslandWindow envia a medição inicial e atualiza nas mudanças de tamanho', () => {
    expect(islandWindowSource).toContain('petHitTestPayload(settings.companion.freeMode');
    expect(islandWindowSource).toContain('sprite?.getBoundingClientRect()');
    expect(islandWindowSource).toContain('new ResizeObserver(updateHitTest)');
    expect(islandWindowSource).toContain('enabled: false');
  });
});
