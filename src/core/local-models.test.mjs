import { describe, expect, it } from 'vitest';
import manifest from '../../src-tauri/resources/local-models.json';
import { isHardwareProfile, isLocalModelInventory, recommendLocalModel } from './local-models';
import { validateManifest } from '../../scripts/refresh-local-models.mjs';

const profile = (ram, vram = 0, avx2 = true) => ({
  total_ram_bytes: ram,
  available_ram_bytes: ram,
  models_free_bytes: 0,
  cpu_cores: 4,
  avx: avx2,
  avx2,
  avx512: false,
  adapters: vram ? [{ name: 'GPU simulada', dedicated_vram_bytes: vram }] : [],
  warnings: [],
});
const gib = 1_073_741_824;

describe('modelos locais', () => {
  it('aceita o perfil de hardware completo e rejeita respostas IPC inválidas', () => {
    expect(isHardwareProfile(profile(16 * gib, 4 * gib))).toBe(true);
    expect(isHardwareProfile({ ...profile(16 * gib), adapters: null })).toBe(false);
    expect(isHardwareProfile({ ...profile(16 * gib), cpu_cores: '8' })).toBe(false);
  });

  it('aceita inventário IPC conhecido e rejeita formato ou estado inválido', () => {
    const entry = {
      modelId: 'modelo-teste',
      name: 'Modelo de teste',
      fileName: 'modelo.gguf',
      sizeBytes: 10,
      status: 'BAIXADO',
    };
    expect(isLocalModelInventory([entry])).toBe(true);
    expect(isLocalModelInventory({ items: [entry] })).toBe(false);
    expect(isLocalModelInventory([{ ...entry, status: 'PRONTO' }])).toBe(false);
  });

  it('recomenda ultraleve e avisa sobre processador sem AVX2', () => {
    const result = recommendLocalModel(profile(6 * gib, 0, false));
    expect(result.profile).toBe('ultraleve');
    expect(result.reason).toContain('não tem AVX2');
  });
  it('recomenda leve em computador com 16 GB sem GPU dedicada', () => {
    expect(recommendLocalModel(profile(16 * gib)).profile).toBe('leve');
  });
  it('recomenda pesado com GPU dedicada de 24 GB e RAM suficiente', () => {
    expect(recommendLocalModel(profile(64 * gib, 24 * gib)).profile).toBe('pesado');
  });
  it('valida o manifesto gerado e rejeita hash inválido', () => {
    expect(validateManifest(manifest)).toBe(true);
    expect(
      validateManifest({
        ...manifest,
        models: manifest.models.map((model, index) =>
          index ? model : { ...model, sha256: 'fake' },
        ),
      }),
    ).toBe(false);
  });
});
