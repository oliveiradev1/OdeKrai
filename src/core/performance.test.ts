import { describe, expect, it } from 'vitest';
import {
  evaluatePerformanceMode,
  LOW_RESOURCE_CPU_CORES,
  LOW_RESOURCE_RAM_BYTES,
} from './performance';

const normalHardware = {
  total_ram_bytes: 16 * 1_073_741_824,
  cpu_cores: 8,
};

describe('regra do modo Desempenho', () => {
  it('liga manualmente sem depender do perfil de hardware', () => {
    expect(evaluatePerformanceMode('enabled', false, null)).toEqual({
      enabled: true,
      reason: 'Ativado manualmente.',
    });
  });

  it('desliga manualmente mesmo quando o sistema pede menos movimento', () => {
    expect(evaluatePerformanceMode('disabled', true, null)).toEqual({
      enabled: false,
      reason: 'Desativado manualmente.',
    });
  });

  it('ativa em Automático quando o Windows solicita movimento reduzido', () => {
    expect(evaluatePerformanceMode('automatic', true, normalHardware)).toMatchObject({
      enabled: true,
      reason: 'O Windows solicita movimento reduzido.',
    });
  });

  it('ativa em Automático no limite de RAM ou no limite de núcleos', () => {
    expect(
      evaluatePerformanceMode('automatic', false, {
        ...normalHardware,
        total_ram_bytes: LOW_RESOURCE_RAM_BYTES,
      }).reason,
    ).toContain('até 8 GiB de RAM');
    const lowCpu = evaluatePerformanceMode('automatic', false, {
      ...normalHardware,
      cpu_cores: LOW_RESOURCE_CPU_CORES,
    });
    expect(lowCpu.enabled).toBe(true);
    expect(lowCpu.reason).toContain('até 4 núcleos lógicos de CPU');
  });

  it('mantém Automático desligado acima dos limites e explica o motivo', () => {
    expect(evaluatePerformanceMode('automatic', false, normalHardware)).toEqual({
      enabled: false,
      reason:
        'Automático desativado: o hardware está acima dos limites e o Windows não solicita movimento reduzido.',
    });
  });

  it('explica quando a consulta de hardware não está disponível', () => {
    const result = evaluatePerformanceMode('automatic', false, null);
    expect(result.enabled).toBe(false);
    expect(result.reason).toContain('não foi possível consultar o hardware');
  });

  it('não trata valores ausentes no perfil como hardware acima dos limites', () => {
    const result = evaluatePerformanceMode('automatic', false, {
      total_ram_bytes: 0,
      cpu_cores: 8,
    });
    expect(result.enabled).toBe(false);
    expect(result.reason).toContain('o perfil não informa RAM');
  });
});
