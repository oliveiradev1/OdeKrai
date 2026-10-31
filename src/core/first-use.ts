export const FIRST_USE_STORAGE_KEY = 'odekrai:first-use:v1';

export type FirstUseStep = 0 | 1 | 2 | 3;

export type FirstUseProgress = {
  autoOpened: boolean;
  currentStep: FirstUseStep;
  completedSteps: FirstUseStep[];
};

export type FirstUseStorage = Pick<Storage, 'getItem' | 'setItem'>;

export const INITIAL_FIRST_USE_PROGRESS: FirstUseProgress = {
  autoOpened: false,
  currentStep: 0,
  completedSteps: [],
};

function isStep(value: unknown): value is FirstUseStep {
  return value === 0 || value === 1 || value === 2 || value === 3;
}

export function loadFirstUseProgress(storage?: FirstUseStorage): FirstUseProgress {
  try {
    const raw = storage?.getItem(FIRST_USE_STORAGE_KEY);
    if (!raw) return { ...INITIAL_FIRST_USE_PROGRESS };
    const value: unknown = JSON.parse(raw);
    if (!value || typeof value !== 'object') return { ...INITIAL_FIRST_USE_PROGRESS };
    const candidate = value as Partial<FirstUseProgress>;
    return {
      autoOpened: candidate.autoOpened === true,
      currentStep: isStep(candidate.currentStep) ? candidate.currentStep : 0,
      completedSteps: Array.isArray(candidate.completedSteps)
        ? [...new Set(candidate.completedSteps.filter(isStep))]
        : [],
    };
  } catch {
    return { ...INITIAL_FIRST_USE_PROGRESS };
  }
}

export function saveFirstUseProgress(
  progress: FirstUseProgress,
  storage?: FirstUseStorage,
): FirstUseProgress {
  try {
    storage?.setItem(FIRST_USE_STORAGE_KEY, JSON.stringify(progress));
  } catch {
    // O assistente segue funcionando mesmo se o armazenamento local estiver indisponível.
  }
  return progress;
}

export function markFirstUseAutoOpened(progress: FirstUseProgress): FirstUseProgress {
  return { ...progress, autoOpened: true };
}

export function shouldAutoOpenFirstUse(progress: FirstUseProgress): boolean {
  return !progress.autoOpened;
}

export function moveFirstUseStep(progress: FirstUseProgress, step: FirstUseStep): FirstUseProgress {
  return { ...progress, currentStep: step };
}

export function markFirstUseStepComplete(
  progress: FirstUseProgress,
  step: FirstUseStep,
): FirstUseProgress {
  return {
    ...progress,
    completedSteps: [...new Set([...progress.completedSteps, step])].sort(),
  };
}

export function setFirstUseStepCompletion(
  progress: FirstUseProgress,
  step: FirstUseStep,
  complete: boolean,
): FirstUseProgress {
  return {
    ...progress,
    completedSteps: complete
      ? [...new Set([...progress.completedSteps, step])].sort()
      : progress.completedSteps.filter((completed) => completed !== step),
  };
}

export function advanceFirstUseStep(
  progress: FirstUseProgress,
  step: FirstUseStep,
  complete: boolean,
): FirstUseProgress {
  const updated = setFirstUseStepCompletion(progress, step, complete);
  return moveFirstUseStep(updated, Math.min(3, step + 1) as FirstUseStep);
}

export function skipFirstUseStep(progress: FirstUseProgress, step: FirstUseStep): FirstUseProgress {
  return moveFirstUseStep(progress, Math.min(3, step + 1) as FirstUseStep);
}

export function firstUseCompletion(progress: FirstUseProgress): number {
  return progress.completedSteps.length;
}
