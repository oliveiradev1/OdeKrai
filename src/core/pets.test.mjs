import { readFileSync } from 'node:fs';
import { inflateSync } from 'node:zlib';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import {
  DEFAULT_PET_ID,
  PET_ANIMATION_LABELS,
  PETS,
  petAnimationForAssistantActivity,
  petAnimationForIslandState,
  petGreeting,
  petAnimationSheet,
  petImageUrl,
  petMotionDisabled,
  petVisualMode,
  validatePetCatalog,
} from './pets';

function pngAlphaBounds(bytes) {
  expect(bytes.subarray(0, 8)).toEqual(Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]));
  const width = bytes.readUInt32BE(16);
  const height = bytes.readUInt32BE(20);
  const bitDepth = bytes[24];
  const colorType = bytes[25];
  const compressed = [];
  for (let offset = 8; offset < bytes.length;) {
    const length = bytes.readUInt32BE(offset);
    const type = bytes.toString('ascii', offset + 4, offset + 8);
    if (type === 'IDAT') compressed.push(bytes.subarray(offset + 8, offset + 8 + length));
    offset += length + 12;
    if (type === 'IEND') break;
  }
  expect(bitDepth).toBe(8);
  expect(colorType).toBe(6);

  const raw = inflateSync(Buffer.concat(compressed));
  const stride = width * 4;
  let previous = Buffer.alloc(stride);
  let transparent = 0;
  let visible = 0;
  for (let y = 0; y < height; y += 1) {
    const rowStart = y * (stride + 1);
    const filter = raw[rowStart];
    const row = Buffer.from(raw.subarray(rowStart + 1, rowStart + 1 + stride));
    for (let index = 0; index < stride; index += 1) {
      const left = index >= 4 ? row[index - 4] : 0;
      const above = previous[index];
      const upperLeft = index >= 4 ? previous[index - 4] : 0;
      if (filter === 1) row[index] = (row[index] + left) & 0xff;
      else if (filter === 2) row[index] = (row[index] + above) & 0xff;
      else if (filter === 3) row[index] = (row[index] + Math.floor((left + above) / 2)) & 0xff;
      else if (filter === 4) {
        const base = left + above - upperLeft;
        const leftDistance = Math.abs(base - left);
        const aboveDistance = Math.abs(base - above);
        const diagonalDistance = Math.abs(base - upperLeft);
        const predictor =
          leftDistance <= aboveDistance && leftDistance <= diagonalDistance
            ? left
            : aboveDistance <= diagonalDistance
              ? above
              : upperLeft;
        row[index] = (row[index] + predictor) & 0xff;
      } else expect(filter).toBe(0);
    }
    for (let index = 3; index < stride; index += 4) {
      if (row[index] === 0) transparent += 1;
      if (row[index] > 0) visible += 1;
    }
    previous = row;
  }
  return { width, height, transparent, visible };
}

function webpInfo(bytes) {
  expect(bytes.toString('ascii', 0, 4)).toBe('RIFF');
  expect(bytes.toString('ascii', 8, 12)).toBe('WEBP');
  let width = 0;
  let height = 0;
  let alpha = false;
  for (let offset = 12; offset + 8 <= bytes.length;) {
    const type = bytes.toString('ascii', offset, offset + 4);
    const length = bytes.readUInt32LE(offset + 4);
    const data = offset + 8;
    if (type === 'VP8X') {
      alpha ||= (bytes[data] & 0x10) !== 0;
      width = 1 + bytes[data + 4] + (bytes[data + 5] << 8) + (bytes[data + 6] << 16);
      height = 1 + bytes[data + 7] + (bytes[data + 8] << 8) + (bytes[data + 9] << 16);
    } else if (type === 'ALPH') {
      alpha = true;
    } else if (type === 'VP8L') {
      const bits = bytes.readUInt32LE(data + 1);
      width = (bits & 0x3fff) + 1;
      height = ((bits >>> 14) & 0x3fff) + 1;
      alpha ||= (bits & 0x10000000) !== 0;
    }
    offset += 8 + length + (length & 1);
  }
  return { width, height, alpha };
}

describe('catálogo e imagens do mascote', () => {
  it('mapeia eventos reais do assistente para animações existentes', () => {
    expect(petAnimationForAssistantActivity('idle')).toBe('idle');
    expect(petAnimationForAssistantActivity('thinking')).toBe('thinking');
    expect(petAnimationForAssistantActivity('task')).toBe('working');
    expect(petAnimationForAssistantActivity('responding')).toBe('working');
    expect(petAnimationForAssistantActivity('awaiting_approval')).toBe('awaiting_approval');
    expect(petAnimationForAssistantActivity('completed')).toBe('success');
    expect(petAnimationForAssistantActivity('error')).toBe('error');
    expect(petAnimationForAssistantActivity('paused')).toBe('paused');
    expect(petAnimationForAssistantActivity('stopped')).toBe('paused');
    expect(petAnimationForAssistantActivity('listening')).toBe('idle');
    expect(petAnimationForAssistantActivity('unknown')).toBe('idle');
  });

  it('rejeita um pets.json ausente ou inválido', () => {
    expect(validatePetCatalog(undefined)).toBeNull();
    expect(validatePetCatalog(null)).toBeNull();
    expect(
      validatePetCatalog([
        { id: 'x', name: 'x', file: '../x.png', description: 'x', altText: 'x' },
      ]),
    ).toBeNull();
    expect(
      validatePetCatalog([{ id: 'x', name: 'x', file: 'x.png', description: 'x', altText: 'x' }]),
    ).toBeNull();
    expect(
      validatePetCatalog(
        Array.from({ length: 12 }, (_, i) => ({
          id: `pet-${i}`,
          name: 'Pet',
          file: `pet-${i}.png`,
          description: 'aparência visual',
          altText: 'Pet',
        })),
      ),
    ).toHaveLength(12);
    expect(
      validatePetCatalog(
        Array.from({ length: 12 }, (_, i) => ({
          id: `pet-${i}`,
          name: 'Pet',
          file: `pet-${i}.png`,
          ...(i === 0 ? {} : { description: 'aparência visual' }),
          altText: 'Pet',
        })),
      ),
    ).toBeNull();
    expect(DEFAULT_PET_ID).toBe('character-female-a');
  });

  it('mantém os IDs e associa os nomes próprios solicitados às aparências visuais', () => {
    const expectedNames = {
      'character-female-a': 'Nina',
      'character-female-b': 'Mel',
      'character-female-c': 'Clara',
      'character-female-d': 'Alice',
      'character-female-e': 'Vera',
      'character-female-f': 'Bia',
      'character-male-a': 'Theo',
      'character-male-b': 'Beto',
      'character-male-c': 'Guto',
      'character-male-d': 'Davi',
      'character-male-e': 'Caio',
      'character-male-f': 'Leo',
    };
    expect(Object.fromEntries(PETS.map(({ id, name }) => [id, name]))).toEqual(expectedNames);
    for (const pet of PETS) {
      expect(pet.description.length).toBeGreaterThan(0);
      expect(pet.altText).toContain(pet.name);
      expect(pet.altText).toContain(pet.description);
    }
    expect(DEFAULT_PET_ID).toBe('character-female-a');
    expect(petGreeting('character-male-d')).toBe('Oi, eu sou o Davi');
    expect(petGreeting('character-female-a')).toBe('Oi, eu sou a Nina');
  });

  it('inclui os 12 arquivos renderizados em 512×512 com transparência', () => {
    expect(PETS).toHaveLength(12);
    for (const pet of PETS) {
      const path = fileURLToPath(
        new URL(`../../assets/pets/kenney-mini-characters/${pet.file}`, import.meta.url),
      );
      const image = pngAlphaBounds(readFileSync(path));
      expect(image.width, pet.file).toBe(512);
      expect(image.height, pet.file).toBe(512);
      expect(image.transparent, pet.file).toBeGreaterThan(0);
      expect(image.visible, pet.file).toBeGreaterThan(0);
    }
  });

  it('mapeia os estados da ilha para o estado visual esperado', () => {
    expect({
      idle: petAnimationForIslandState('compact'),
      thinking: petAnimationForIslandState('thinking'),
      working: petAnimationForIslandState('responding'),
      approval: petAnimationForIslandState('awaiting_approval'),
      success: petAnimationForIslandState('completed'),
      error: petAnimationForIslandState('error'),
      paused: petAnimationForIslandState('paused'),
      unknown: petAnimationForIslandState('unmapped'),
    }).toEqual({
      idle: 'idle',
      thinking: 'thinking',
      working: 'working',
      approval: 'awaiting_approval',
      success: 'success',
      error: 'error',
      paused: 'paused',
      unknown: 'idle',
    });
  });

  it('mapeia cada estado do pet para um rótulo', () => {
    expect(PET_ANIMATION_LABELS).toEqual({
      idle: 'Em espera',
      thinking: 'Pensando',
      working: 'Trabalhando',
      awaiting_approval: 'Aguardando aprovação',
      success: 'Concluído',
      error: 'Erro',
      paused: 'Pausado',
    });
  });

  it('retorna fallback para um ID sem PNG no catálogo e preserva o movimento reduzido', () => {
    expect(petImageUrl('character-missing')).toBeNull();
    expect(petImageUrl('character-male-a')).toMatch(/\.png$/);
    expect(petAnimationSheet('character-missing', 'idle')).toBeNull();
    expect(petVisualMode(false, Boolean(petImageUrl('character-female-a')))).toBe('static');
    expect(petVisualMode(false, false)).toBe('placeholder');
    expect(petVisualMode(true, true)).toBe('animated');
    expect(petMotionDisabled(true, false)).toBe(true);
    expect(petMotionDisabled(false, true)).toBe(true);
    expect(petMotionDisabled(false, false, true)).toBe(true);
    expect(petMotionDisabled(false, false)).toBe(false);
  });

  it('valida manifestos, mapeamento para folhas, dimensões, transparência e orçamento', () => {
    const sheetsSeen = new Set();
    let totalBytes = 0;
    const states = [
      'idle',
      'thinking',
      'working',
      'awaiting_approval',
      'success',
      'error',
      'paused',
    ];
    const expectedClips = {
      idle: 'idle',
      thinking: 'idle',
      working: 'interact-right',
      awaiting_approval: 'emote-no',
      success: 'emote-yes',
      error: 'die',
      paused: 'sit',
    };
    for (const pet of PETS) {
      const manifestPath = fileURLToPath(
        new URL(
          `../../assets/pets/kenney-mini-characters/animations/${pet.id}.json`,
          import.meta.url,
        ),
      );
      const manifest = JSON.parse(readFileSync(manifestPath, 'utf8'));
      expect(manifest.id).toBe(pet.id);
      expect(manifest.frameWidth).toBe(192);
      expect(manifest.frameHeight).toBe(192);
      for (const state of states) {
        const sheet = petAnimationSheet(pet.id, state);
        const entry = manifest.animations[state];
        expect(sheet, `${pet.id}/${state}`).not.toBeNull();
        expect(entry.clip).toBe(expectedClips[state]);
        expect(entry.frames).toBeGreaterThanOrEqual(8);
        expect(entry.frames).toBeLessThanOrEqual(12);
        expect(entry.fps).toBeGreaterThan(0);
        expect(typeof entry.loop).toBe('boolean');
        expect(entry.loop).toBe(!['success', 'error'].includes(state));
        expect(sheet.url).toBeTruthy();
        const path = fileURLToPath(
          new URL(
            `../../assets/pets/kenney-mini-characters/animations/${entry.file}`,
            import.meta.url,
          ),
        );
        const bytes = readFileSync(path);
        if (!sheetsSeen.has(path)) totalBytes += bytes.length;
        sheetsSeen.add(path);
        const info = webpInfo(bytes);
        expect(info.width).toBe(entry.frames * 192);
        expect(info.height).toBe(192);
        expect(info.alpha).toBe(true);
      }
    }
    expect(totalBytes).toBeLessThanOrEqual(6 * 1024 * 1024);
  });
});
