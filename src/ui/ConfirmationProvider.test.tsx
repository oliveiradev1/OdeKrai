import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import { ConfirmationDialogContent } from './ConfirmationProvider';
import { focusInitialElement } from './Modal';

describe('diálogo interno de confirmação', () => {
  it('marca Cancelar como foco inicial e destaca confirmação destrutiva', () => {
    const markup = renderToStaticMarkup(
      <ConfirmationDialogContent
        request={{ title: 'Apagar memória?', message: 'Apagar notas.md?', destructive: true }}
        onCancel={vi.fn()}
        onConfirm={vi.fn()}
      />,
    );

    expect(markup).toContain('Apagar notas.md?');
    expect(markup).toContain('data-autofocus=""');
    expect(markup).toContain('ui-button--danger');
    expect(markup).toContain('Cancelar');
    expect(markup).toContain('Confirmar');
  });

  it('não aplica foco inicial destrutivo em confirmação comum', () => {
    const markup = renderToStaticMarkup(
      <ConfirmationDialogContent
        request={{ title: 'Salvar identidade?', message: 'Salvar agora?' }}
        onCancel={vi.fn()}
        onConfirm={vi.fn()}
      />,
    );
    expect(markup).not.toContain('data-autofocus');
    expect(markup).toContain('ui-button--primary');
  });

  it('envia o foco para Cancelar quando a ação é destrutiva', () => {
    const cancel = {
      tabIndex: 0,
      focus: vi.fn(),
      matches: () => false,
      closest: () => null,
    };
    const confirm = {
      tabIndex: 0,
      focus: vi.fn(),
      matches: () => false,
      closest: () => null,
    };
    const dialog = {
      querySelector: () => cancel,
      querySelectorAll: () => [cancel, confirm],
      focus: vi.fn(),
    } as unknown as HTMLElement;

    focusInitialElement(dialog);
    expect(cancel.focus).toHaveBeenCalledOnce();
    expect(confirm.focus).not.toHaveBeenCalled();
  });
});
