import { describe, expect, it, vi } from 'vitest';
import { submitSecretForm } from './secret-form';

describe('submissão de campos secretos', () => {
  it('cancela a navegação do formulário, salva e limpa após sucesso', async () => {
    const preventDefault = vi.fn();
    const save = vi.fn().mockResolvedValue(true);
    const clear = vi.fn();

    await submitSecretForm({ preventDefault }, save, clear);

    expect(preventDefault).toHaveBeenCalledOnce();
    expect(save).toHaveBeenCalledOnce();
    expect(clear).toHaveBeenCalledOnce();
  });

  it('mantém o valor se a gravação falhar', async () => {
    const clear = vi.fn();

    await submitSecretForm({ preventDefault: vi.fn() }, async () => false, clear);

    expect(clear).not.toHaveBeenCalled();
  });
});
