import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import { FirstUseStepNavigation, type FirstUseStepContent } from './FirstUseAssistant';

const steps: FirstUseStepContent[] = [
  { id: 0, title: 'Provedor', description: '', complete: true, content: null },
  { id: 1, title: 'Pasta', description: '', complete: false, content: null },
  { id: 2, title: 'Mascote', description: '', complete: false, content: null },
  { id: 3, title: 'Permissões', description: '', complete: false, content: null },
];

describe('navegação do assistente de primeiro uso', () => {
  it('expõe etapas como botões acessíveis e anuncia a etapa atual', () => {
    const markup = renderToStaticMarkup(
      <FirstUseStepNavigation
        steps={steps}
        currentStep={1}
        completedSteps={[0]}
        onSelect={vi.fn()}
      />,
    );
    expect(markup).toContain('aria-label="Etapas de configuração"');
    expect(markup).toContain('aria-current="step"');
    expect(markup).toContain('aria-label="Provedor, concluída"');
    expect(markup.match(/<button/g)).toHaveLength(4);
  });
});
