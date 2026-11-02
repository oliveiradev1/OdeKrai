import { describe, expect, it, vi } from 'vitest';
import { runOnceWhilePending } from './single-flight';

describe('runOnceWhilePending', () => {
  it('shares concurrent work and allows a later refresh', async () => {
    const slot: { current: Promise<string> | null } = { current: null };
    let finishRequest: (value: string) => void = () => undefined;
    const run = vi.fn(
      () =>
        new Promise<string>((resolve) => {
          finishRequest = resolve;
        }),
    );

    const first = runOnceWhilePending(slot, run);
    const second = runOnceWhilePending(slot, run);
    expect(run).toHaveBeenCalledTimes(1);

    finishRequest('carregado');
    await expect(Promise.all([first, second])).resolves.toEqual(['carregado', 'carregado']);
    expect(slot.current).toBeNull();

    const refresh = runOnceWhilePending(slot, async () => 'atualizado');
    await expect(refresh).resolves.toBe('atualizado');
  });
});
