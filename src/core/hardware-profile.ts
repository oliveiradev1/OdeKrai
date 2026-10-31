function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isNonNegativeSafeInteger(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0;
}

export type HardwareProfile = {
  total_ram_bytes: number;
  available_ram_bytes: number;
  models_free_bytes: number;
  cpu_cores: number;
  avx: boolean;
  avx2: boolean;
  avx512: boolean;
  adapters: { name: string; dedicated_vram_bytes: number }[];
  warnings: string[];
};

export function isHardwareProfile(value: unknown): value is HardwareProfile {
  if (!isRecord(value)) return false;
  return (
    isNonNegativeSafeInteger(value.total_ram_bytes) &&
    isNonNegativeSafeInteger(value.available_ram_bytes) &&
    isNonNegativeSafeInteger(value.models_free_bytes) &&
    isNonNegativeSafeInteger(value.cpu_cores) &&
    typeof value.avx === 'boolean' &&
    typeof value.avx2 === 'boolean' &&
    typeof value.avx512 === 'boolean' &&
    Array.isArray(value.adapters) &&
    value.adapters.every(
      (adapter) =>
        isRecord(adapter) &&
        typeof adapter.name === 'string' &&
        isNonNegativeSafeInteger(adapter.dedicated_vram_bytes),
    ) &&
    Array.isArray(value.warnings) &&
    value.warnings.every((warning) => typeof warning === 'string')
  );
}
