export type PerformanceMode = 'automatic' | 'enabled' | 'disabled';

export type PerformanceHardwareProfile = {
  total_ram_bytes: number;
  cpu_cores: number;
};

export type PerformanceModeStatus = {
  enabled: boolean;
  reason: string;
};

export const LOW_RESOURCE_RAM_BYTES = 8 * 1_073_741_824;
export const LOW_RESOURCE_CPU_CORES = 4;

export function evaluatePerformanceMode(
  mode: PerformanceMode,
  prefersReducedMotion: boolean,
  hardware: PerformanceHardwareProfile | null,
): PerformanceModeStatus {
  if (mode === 'enabled') return { enabled: true, reason: 'Ativado manualmente.' };
  if (mode === 'disabled') return { enabled: false, reason: 'Desativado manualmente.' };

  const reasons: string[] = [];
  if (prefersReducedMotion) reasons.push('O Windows solicita movimento reduzido.');

  if (
    hardware &&
    hardware.total_ram_bytes > 0 &&
    hardware.total_ram_bytes <= LOW_RESOURCE_RAM_BYTES
  ) {
    reasons.push('O computador tem até 8 GiB de RAM.');
  }
  if (hardware && hardware.cpu_cores > 0 && hardware.cpu_cores <= LOW_RESOURCE_CPU_CORES) {
    reasons.push('O computador tem até 4 núcleos lógicos de CPU.');
  }

  if (reasons.length > 0) return { enabled: true, reason: reasons.join(' ') };
  if (!hardware) {
    return {
      enabled: false,
      reason:
        'Automático desativado: não foi possível consultar o hardware e o Windows não solicita movimento reduzido.',
    };
  }
  const unavailableResources = [
    hardware.total_ram_bytes <= 0 ? 'RAM' : null,
    hardware.cpu_cores <= 0 ? 'núcleos de CPU' : null,
  ].filter((resource): resource is string => resource !== null);
  if (unavailableResources.length > 0) {
    return {
      enabled: false,
      reason: `Automático desativado: o perfil não informa ${unavailableResources.join(' ou ')} e o Windows não solicita movimento reduzido.`,
    };
  }
  return {
    enabled: false,
    reason:
      'Automático desativado: o hardware está acima dos limites e o Windows não solicita movimento reduzido.',
  };
}
