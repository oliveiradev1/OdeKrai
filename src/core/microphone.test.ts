import { describe, expect, it, vi } from 'vitest';
import { activateMicrophone, microphoneClickAction, microphonePermissionError } from './microphone';

describe('fluxo do microfone', () => {
  it('mostra ativação antes da gravação quando a preferência está desligada', () => {
    expect(microphoneClickAction(false)).toBe('activate');
    expect(microphoneClickAction(true)).toBe('record');
  });

  it('persiste a preferência antes de pedir acesso e só então inicia a gravação', async () => {
    const calls: string[] = [];
    const stream = {} as MediaStream;
    const beginRecording = vi.fn(() => calls.push('gravar'));

    await activateMicrophone(
      () => {
        calls.push('persistir');
      },
      async () => {
        calls.push('permissão');
        return stream;
      },
      (allowedStream) => {
        expect(allowedStream).toBe(stream);
        return beginRecording();
      },
    );

    expect(calls).toEqual(['persistir', 'permissão', 'gravar']);
    expect(beginRecording).toHaveBeenCalledOnce();
  });

  it('não inicia a gravação quando o acesso é negado e orienta a liberar no Windows', async () => {
    const beginRecording = vi.fn();
    const denial = new DOMException('blocked', 'NotAllowedError');

    await expect(
      activateMicrophone(
        () => undefined,
        async () => {
          throw denial;
        },
        beginRecording,
      ),
    ).rejects.toBe(denial);

    expect(beginRecording).not.toHaveBeenCalled();
    expect(microphonePermissionError(denial)).toContain(
      'Configurações do Windows → Privacidade e segurança → Microfone',
    );
  });
});
