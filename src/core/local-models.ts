import manifest from '../../src-tauri/resources/local-models.json';
import type { HardwareProfile } from './hardware-profile';

export { isHardwareProfile, type HardwareProfile } from './hardware-profile';

export type LocalModelInventory = {
  modelId: string;
  name: string;
  fileName: string;
  path: string;
  sizeBytes: number;
  status: 'NAO_BAIXADO' | 'BAIXANDO' | 'PARCIAL' | 'BAIXADO' | 'VERIFICADO' | 'ERRO';
  engine: string;
  ramVramEstimate: string;
};

const inventoryStatuses = new Set<LocalModelInventory['status']>([
  'NAO_BAIXADO',
  'BAIXANDO',
  'PARCIAL',
  'BAIXADO',
  'VERIFICADO',
  'ERRO',
]);

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

export function isLocalModelInventory(value: unknown): value is LocalModelInventory[] {
  return (
    Array.isArray(value) &&
    value.every(
      (entry) =>
        isRecord(entry) &&
        typeof entry.modelId === 'string' &&
        typeof entry.name === 'string' &&
        typeof entry.fileName === 'string' &&
        typeof entry.path === 'string' &&
        isNonNegativeSafeInteger(entry.sizeBytes) &&
        typeof entry.status === 'string' &&
        inventoryStatuses.has(entry.status as LocalModelInventory['status']) &&
        typeof entry.engine === 'string' &&
        typeof entry.ramVramEstimate === 'string',
    )
  );
}

export const LOCAL_MODELS = manifest.models;

export function recommendLocalModel(hardware: HardwareProfile) {
  const vram = Math.max(0, ...hardware.adapters.map((adapter) => adapter.dedicated_vram_bytes));
  const availableRamGiB = hardware.available_ram_bytes / 1_073_741_824;
  const profile =
    vram >= 20 * 1_073_741_824 && availableRamGiB >= 32
      ? 'pesado'
      : availableRamGiB >= 20 || vram >= 8 * 1_073_741_824
        ? 'medio'
        : availableRamGiB >= 8
          ? 'leve'
          : 'ultraleve';
  const alternatives =
    profile === 'pesado'
      ? ['medio', 'leve']
      : profile === 'medio'
        ? ['leve', 'pesado']
        : profile === 'leve'
          ? ['ultraleve', 'medio']
          : ['leve'];
  const reason = `${profile === 'pesado' ? 'A memória disponível e a VRAM favorecem um modelo pesado.' : profile === 'medio' ? 'A memória disponível favorece um modelo médio.' : profile === 'leve' ? 'A memória disponível favorece um modelo leve.' : 'A memória disponível é limitada; comece por um modelo ultraleve.'}${hardware.avx2 ? '' : ' Este processador não tem AVX2; a execução pode ser lenta.'}`;
  return { profile, alternatives, reason };
}

export function formatBytes(bytes: number | null) {
  if (bytes === null) return 'Não verificado';
  return `${(bytes / 1_000_000_000).toFixed(2)} GB`;
}

function isNonNegativeSafeInteger(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0;
}
