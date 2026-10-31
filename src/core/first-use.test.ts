import { describe, expect, it } from 'vitest';
import {
  advanceFirstUseStep,
  FIRST_USE_STORAGE_KEY,
  INITIAL_FIRST_USE_PROGRESS,
  firstUseCompletion,
  loadFirstUseProgress,
  markFirstUseAutoOpened,
  markFirstUseStepComplete,
  moveFirstUseStep,
  saveFirstUseProgress,
  setFirstUseStepCompletion,
  shouldAutoOpenFirstUse,
  skipFirstUseStep,
  type FirstUseStorage,
} from './first-use';

function mockStorage(): FirstUseStorage {
  const values = new Map<string, string>();
  return {
    getItem: (key) => values.get(key) ?? null,
    setItem: (key, value) => values.set(key, value),
  };
}

describe('assistente de primeiro uso', () => {
  it('persiste retomada, passo atual e conclusão sem guardar dados de provedores', () => {
    const storage = mockStorage();
    const progress = markFirstUseStepComplete(moveFirstUseStep(INITIAL_FIRST_USE_PROGRESS, 2), 0);
    saveFirstUseProgress(progress, storage);

    expect(loadFirstUseProgress(storage)).toEqual({
      autoOpened: false,
      currentStep: 2,
      completedSteps: [0],
    });
    expect(storage.getItem(FIRST_USE_STORAGE_KEY)).not.toContain('credential');
  });

  it('permite pular e voltar entre passos sem marcá-los como concluídos', () => {
    const skipped = moveFirstUseStep(INITIAL_FIRST_USE_PROGRESS, 2);
    expect(skipped.currentStep).toBe(2);
    expect(skipped.completedSteps).toEqual([]);
    expect(moveFirstUseStep(skipped, 1).currentStep).toBe(1);
  });

  it('abre automaticamente apenas uma vez e permite concluir cada etapa uma vez', () => {
    const opened = markFirstUseAutoOpened(INITIAL_FIRST_USE_PROGRESS);
    const completed = markFirstUseStepComplete(markFirstUseStepComplete(opened, 1), 1);
    expect(loadFirstUseProgress(mockStorage()).autoOpened).toBe(false);
    expect(opened.autoOpened).toBe(true);
    expect(shouldAutoOpenFirstUse(INITIAL_FIRST_USE_PROGRESS)).toBe(true);
    expect(shouldAutoOpenFirstUse(opened)).toBe(false);
    expect(completed.completedSteps).toEqual([1]);
    expect(firstUseCompletion(completed)).toBe(1);
  });

  it('avança apenas marca passos realmente concluídos, enquanto pular preserva a pendência', () => {
    const completed = advanceFirstUseStep(INITIAL_FIRST_USE_PROGRESS, 0, true);
    expect(completed).toMatchObject({ currentStep: 1, completedSteps: [0] });
    const skipped = skipFirstUseStep(INITIAL_FIRST_USE_PROGRESS, 0);
    expect(skipped).toMatchObject({ currentStep: 1, completedSteps: [] });
    expect(moveFirstUseStep(skipped, 0).currentStep).toBe(0);
  });

  it('remove a conclusão persistida se um requisito deixar de existir', () => {
    const completed = markFirstUseStepComplete(INITIAL_FIRST_USE_PROGRESS, 0);
    expect(setFirstUseStepCompletion(completed, 0, false).completedSteps).toEqual([]);
  });

  it('percorre etapas com recursos simulados, permite pular e retomar no passo salvo', () => {
    const storage = mockStorage();
    let progress = markFirstUseAutoOpened(INITIAL_FIRST_USE_PROGRESS);
    progress = advanceFirstUseStep(progress, 0, true);
    progress = skipFirstUseStep(progress, 1);
    progress = advanceFirstUseStep(progress, 2, true);
    progress = moveFirstUseStep(progress, 3);
    saveFirstUseProgress(progress, storage);

    const resumed = loadFirstUseProgress(storage);
    expect(resumed).toMatchObject({
      autoOpened: true,
      currentStep: 3,
      completedSteps: [0, 2],
    });
    expect(firstUseCompletion(resumed)).toBe(2);
  });

  it('recupera o estado padrão diante de JSON inválido ou dados fora do contrato', () => {
    const storage: FirstUseStorage = {
      getItem: () => '{',
      setItem: () => undefined,
    };
    expect(loadFirstUseProgress(storage)).toEqual(INITIAL_FIRST_USE_PROGRESS);
  });
});
