import { useEffect, useMemo, useState, type ReactNode } from 'react';
import {
  advanceFirstUseStep,
  firstUseCompletion,
  loadFirstUseProgress,
  markFirstUseStepComplete,
  markFirstUseAutoOpened,
  moveFirstUseStep,
  saveFirstUseProgress,
  setFirstUseStepCompletion,
  skipFirstUseStep,
  type FirstUseProgress,
  type FirstUseStep,
} from '../core/first-use';
import { Button } from './DesignSystem';
import { Modal } from './Modal';

export type FirstUseStepContent = {
  id: FirstUseStep;
  title: string;
  description: string;
  complete: boolean;
  content: ReactNode;
};

export function FirstUseStepNavigation({
  steps,
  currentStep,
  completedSteps,
  onSelect,
}: {
  steps: FirstUseStepContent[];
  currentStep: FirstUseStep;
  completedSteps: FirstUseStep[];
  onSelect: (step: FirstUseStep) => void;
}) {
  return (
    <nav className="first-use-step-list" aria-label="Etapas de configuração">
      {steps.map((step) => {
        const completed = completedSteps.includes(step.id) || step.complete;
        return (
          <Button
            type="button"
            key={step.id}
            aria-current={currentStep === step.id ? 'step' : undefined}
            aria-label={`${step.title}${completed ? ', concluída' : ''}`}
            className={currentStep === step.id ? 'active' : ''}
            onClick={() => onSelect(step.id)}
          >
            <span>{step.id + 1}</span>
            <span>{step.title}</span>
            {completed && <span aria-hidden="true">✓</span>}
          </Button>
        );
      })}
    </nav>
  );
}

function storage() {
  return typeof localStorage === 'undefined' ? undefined : localStorage;
}

export function FirstUseAssistant({
  open,
  steps,
  onClose,
  onOpenDocumentation,
}: {
  open: boolean;
  steps: FirstUseStepContent[];
  onClose: () => void;
  onOpenDocumentation: () => void;
}) {
  const [progress, setProgress] = useState<FirstUseProgress>(() => loadFirstUseProgress(storage()));
  const currentStep = steps.find((step) => step.id === progress.currentStep) ?? steps[0];
  const completedCount = firstUseCompletion(progress);
  const completionLabel = useMemo(
    () => `${completedCount} de ${steps.length} etapas concluídas`,
    [completedCount, steps.length],
  );

  useEffect(() => {
    if (!open) return;
    setProgress((current) => {
      let updated = markFirstUseAutoOpened(current);
      if (currentStep) {
        updated = setFirstUseStepCompletion(updated, currentStep.id, currentStep.complete);
      }
      return saveFirstUseProgress(updated, storage());
    });
  }, [open, currentStep?.complete, currentStep?.id]);

  function goTo(step: FirstUseStep) {
    setProgress((current) => {
      const updated = currentStep?.complete
        ? markFirstUseStepComplete(current, currentStep.id)
        : current;
      return saveFirstUseProgress(moveFirstUseStep(updated, step), storage());
    });
  }

  function next() {
    if (currentStep.id < 3)
      setProgress((current) =>
        saveFirstUseProgress(
          advanceFirstUseStep(current, currentStep.id, currentStep.complete),
          storage(),
        ),
      );
    else onClose();
  }

  function skip() {
    if (currentStep.id < 3)
      setProgress((current) =>
        saveFirstUseProgress(skipFirstUseStep(current, currentStep.id), storage()),
      );
    else onClose();
  }

  return (
    <Modal open={open} title="Primeiros passos no OdeKrai" onClose={onClose}>
      <section className="first-use-wizard" aria-label="Assistente de primeiro uso">
        <div className="first-use-progress" role="status" aria-live="polite">
          <strong>{completionLabel}</strong>
          <span>Você pode pular uma etapa e voltar a ela depois.</span>
        </div>
        <FirstUseStepNavigation
          steps={steps}
          currentStep={currentStep.id}
          completedSteps={progress.completedSteps}
          onSelect={goTo}
        />
        {currentStep && (
          <div className="first-use-stage" key={currentStep.id}>
            <header>
              <h3>{currentStep.title}</h3>
              <p>{currentStep.description}</p>
            </header>
            <div className="first-use-step-content">{currentStep.content}</div>
          </div>
        )}
        <footer className="first-use-footer">
          <Button type="button" variant="ghost" onClick={onOpenDocumentation}>
            Ajuda e documentação
          </Button>
          <div>
            <Button
              type="button"
              variant="secondary"
              disabled={currentStep?.id === 0}
              onClick={() => goTo(((currentStep?.id ?? 0) - 1) as FirstUseStep)}
            >
              Voltar
            </Button>
            <Button type="button" variant="ghost" onClick={skip}>
              Pular etapa
            </Button>
            <Button type="button" variant="primary" onClick={next}>
              {currentStep?.id === 3 ? 'Concluir por enquanto' : 'Continuar'}
            </Button>
          </div>
        </footer>
      </section>
    </Modal>
  );
}
