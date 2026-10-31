import { describe, expect, it, vi } from 'vitest';
import { memoryDeletionConfirmation, runConfirmedAction } from './confirmation';

describe('confirmação de ações', () => {
  it('executa a ação somente depois da confirmação', async () => {
    const confirm = vi.fn().mockResolvedValue(true);
    const action = vi.fn();
    const request = { title: 'Confirmar', message: 'Prosseguir?' };

    await expect(runConfirmedAction(confirm, request, action)).resolves.toBe(true);
    expect(confirm).toHaveBeenCalledWith(request);
    expect(action).toHaveBeenCalledOnce();
  });

  it('não executa a ação quando a confirmação é cancelada', async () => {
    const action = vi.fn();

    await expect(
      runConfirmedAction(
        vi.fn().mockResolvedValue(false),
        { title: 'Apagar', message: '?' },
        action,
      ),
    ).resolves.toBe(false);
    expect(action).not.toHaveBeenCalled();
  });

  it('apagar uma memória sempre pede confirmação destrutiva com o nome correto', () => {
    expect(memoryDeletionConfirmation('notas.md')).toEqual({
      title: 'Apagar memória?',
      message: 'Apagar notas.md?',
      destructive: true,
    });
  });
});
