import catalogJson from '../../assets/pets/kenney-mini-characters/pets.json';

export type PetCatalogEntry = {
  id: string;
  name: string;
  file: string;
  description: string;
  altText: string;
};

export type PetAnimationState =
  'idle' | 'thinking' | 'working' | 'awaiting_approval' | 'success' | 'error' | 'paused';

export type PetAnimationSheet = {
  clip: string;
  file: string;
  frames: number;
  fps: number;
  loop: boolean;
};

export type PetAnimationManifest = {
  id: string;
  frameWidth: number;
  frameHeight: number;
  animations: Record<PetAnimationState, PetAnimationSheet>;
};

export const PET_ANIMATION_LABELS: Record<PetAnimationState, string> = {
  idle: 'Em espera',
  thinking: 'Pensando',
  working: 'Trabalhando',
  awaiting_approval: 'Aguardando aprovação',
  success: 'Concluído',
  error: 'Erro',
  paused: 'Pausado',
};

export const DEFAULT_PET_ID = 'character-female-a';

export function validatePetCatalog(value: unknown): PetCatalogEntry[] | null {
  if (!Array.isArray(value) || value.length !== 12) return null;
  const ids = new Set<string>();
  for (const entry of value) {
    if (!entry || typeof entry !== 'object') return null;
    const pet = entry as Partial<PetCatalogEntry>;
    if (
      typeof pet.id !== 'string' ||
      !/^[a-z0-9-]+$/.test(pet.id) ||
      ids.has(pet.id) ||
      typeof pet.name !== 'string' ||
      !pet.name.trim() ||
      typeof pet.description !== 'string' ||
      !pet.description.trim() ||
      typeof pet.altText !== 'string' ||
      !pet.altText.trim() ||
      typeof pet.file !== 'string' ||
      !/^[a-z0-9-]+\.png$/.test(pet.file)
    ) {
      return null;
    }
    ids.add(pet.id);
  }
  return value as PetCatalogEntry[];
}

export const PETS = validatePetCatalog(catalogJson) ?? [];

export function petGreeting(petId: string): string {
  const pet = PETS.find((entry) => entry.id === petId);
  if (!pet) return 'Oi, eu sou seu mascote';
  const article = petId.includes('-female-') ? 'a' : 'o';
  return `Oi, eu sou ${article} ${pet.name}`;
}

const petAssets = import.meta.glob<string>('../../assets/pets/kenney-mini-characters/*.png', {
  eager: true,
  query: '?url',
  import: 'default',
});
const animationAssets = import.meta.glob<string>(
  '../../assets/pets/kenney-mini-characters/animations/*.webp',
  { eager: true, query: '?url', import: 'default' },
);
const animationManifests = import.meta.glob<PetAnimationManifest>(
  '../../assets/pets/kenney-mini-characters/animations/*.json',
  { eager: true, import: 'default' },
);

export function petImageUrl(petId: string): string | null {
  const pet = PETS.find((entry) => entry.id === petId);
  if (!pet || !/^[a-z0-9-]+\.png$/.test(pet.file)) return null;
  return petAssets[`../../assets/pets/kenney-mini-characters/${pet.file}`] ?? null;
}

export function petAnimationSheet(petId: string, state: PetAnimationState) {
  const loadedManifest =
    animationManifests[`../../assets/pets/kenney-mini-characters/animations/${petId}.json`];
  const manifest =
    (loadedManifest as unknown as { default?: PetAnimationManifest } | undefined)?.default ??
    loadedManifest;
  const sheet = manifest?.animations?.[state];
  if (
    manifest?.id !== petId ||
    manifest.frameWidth !== 192 ||
    manifest.frameHeight !== 192 ||
    !sheet ||
    typeof sheet.clip !== 'string' ||
    !/^[a-z0-9-]+\.webp$/.test(sheet.file) ||
    !Number.isInteger(sheet.frames) ||
    sheet.frames < 1 ||
    sheet.frames > 12 ||
    !Number.isFinite(sheet.fps) ||
    sheet.fps <= 0 ||
    typeof sheet.loop !== 'boolean'
  ) {
    return null;
  }
  const loadedUrl =
    animationAssets[`../../assets/pets/kenney-mini-characters/animations/${sheet.file}`];
  const url = (loadedUrl as unknown as { default?: string } | undefined)?.default ?? loadedUrl;
  return url
    ? { ...sheet, url, frameWidth: manifest.frameWidth, frameHeight: manifest.frameHeight }
    : null;
}

export function petVisualMode(hasAnimationSheet: boolean, hasStaticImage: boolean) {
  if (hasAnimationSheet) return 'animated';
  if (hasStaticImage) return 'static';
  return 'placeholder';
}

const islandStateAnimation: Record<string, PetAnimationState> = {
  compact: 'idle',
  expanded: 'idle',
  retracted: 'idle',
  hidden: 'paused',
  listening: 'idle',
  thinking: 'thinking',
  responding: 'working',
  task: 'working',
  notification: 'idle',
  awaiting_approval: 'awaiting_approval',
  paused: 'paused',
  error: 'error',
  completed: 'success',
};

export function petAnimationForIslandState(state: string): PetAnimationState {
  return islandStateAnimation[state] ?? 'idle';
}

const assistantActivityAnimation: Record<string, PetAnimationState> = {
  idle: 'idle',
  thinking: 'thinking',
  task: 'working',
  responding: 'working',
  awaiting_approval: 'awaiting_approval',
  completed: 'success',
  error: 'error',
  paused: 'paused',
  stopped: 'paused',
};

export function petAnimationForAssistantActivity(state: string): PetAnimationState {
  return assistantActivityAnimation[state] ?? 'idle';
}

export function petMotionDisabled(
  reduceMotion: boolean,
  prefersReducedMotion: boolean,
  performanceMode = false,
): boolean {
  return reduceMotion || prefersReducedMotion || performanceMode;
}
