import { describe, expect, it, vi } from 'vitest';
import { runVerification, verificationSteps } from './verify.mjs';

describe('npm run verify', () => {
  it('executa todas as etapas na ordem definida', () => {
    const execute = vi.fn(() => ({ status: 0 }));
    const report = vi.fn();

    expect(runVerification(execute, report)).toBe(0);
    expect(execute.mock.calls.map(([step]) => step.name)).toEqual(
      verificationSteps.map((step) => step.name),
    );
    expect(report).toHaveBeenLastCalledWith('[verify] Todas as etapas passaram.');
  });

  it('mostra a etapa e para no primeiro erro', () => {
    const failedIndex = 3;
    const failedStep = verificationSteps[failedIndex];
    const failedCommand = [failedStep.command, ...failedStep.args].join(' ');
    const execute = vi.fn((_step, index) => ({ status: index === failedIndex ? 2 : 0 }));
    const report = vi.fn();

    expect(runVerification((step) => execute(step, execute.mock.calls.length), report)).toBe(1);
    expect(execute).toHaveBeenCalledTimes(failedIndex + 1);
    expect(report).toHaveBeenCalledWith(`[verify] FALHOU: ${failedStep.name} (${failedCommand})`);
    expect(report).toHaveBeenCalledWith('[verify] Código de saída: 2');
  });

  it('mostra o erro exato quando uma ferramenta não inicia', () => {
    const report = vi.fn();
    const error = new Error('spawn cargo ENOENT');
    const firstStep = verificationSteps[0];
    const firstCommand = [firstStep.command, ...firstStep.args].join(' ');

    expect(runVerification(() => ({ status: null, error }), report)).toBe(1);
    expect(report).toHaveBeenCalledWith(`[verify] FALHOU: ${firstStep.name} (${firstCommand})`);
    expect(report).toHaveBeenCalledWith('[verify] Erro ao iniciar: spawn cargo ENOENT');
  });
});
