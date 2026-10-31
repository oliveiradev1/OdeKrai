import { describe, expect, it } from 'vitest';
import { petAnimationPlayState, shouldPausePetAnimation } from './pet-motion';
import petMotionHookSource from '../ui/usePetMotionActivity.ts?raw';

const active = {
  documentVisible: true,
  windowVisible: true,
  minimized: false,
  focused: true,
  unfocusedForMs: 0,
};

describe('pausa das animações do mascote', () => {
  it('pausa quando o documento ou janela não está visível ou está minimizada', () => {
    expect(shouldPausePetAnimation({ ...active, documentVisible: false })).toBe(true);
    expect(shouldPausePetAnimation({ ...active, windowVisible: false })).toBe(true);
    expect(shouldPausePetAnimation({ ...active, minimized: true })).toBe(true);
  });

  it('aguarda alguns segundos sem foco e retoma assim que a janela volta', () => {
    expect(shouldPausePetAnimation({ ...active, focused: false, unfocusedForMs: 2499 })).toBe(
      false,
    );
    expect(shouldPausePetAnimation({ ...active, focused: false, unfocusedForMs: 2500 })).toBe(true);
    expect(shouldPausePetAnimation(active)).toBe(false);
  });

  it('mantém parado quando a animação é desativada por preferência', () => {
    expect(petAnimationPlayState(false, true)).toBe('paused');
    expect(petAnimationPlayState(true, false)).toBe('paused');
    expect(petAnimationPlayState(false, false)).toBe('running');
  });

  it('acompanha visibilidade e foco por eventos, sem polling contínuo', () => {
    expect(petMotionHookSource).toContain("addEventListener('visibilitychange'");
    expect(petMotionHookSource).toContain('onFocusChanged');
    expect(petMotionHookSource).toContain('.isMinimized()');
    expect(petMotionHookSource).toContain('PET_UNFOCUSED_PAUSE_DELAY_MS');
    expect(petMotionHookSource).toContain("attributeFilter: ['data-performance']");
    expect(petMotionHookSource).not.toContain('setInterval(');
  });
});
