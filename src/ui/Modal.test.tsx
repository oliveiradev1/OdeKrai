import { readFileSync } from 'node:fs';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import {
  focusInitialElement,
  handleBackdropMouseDown,
  handleModalKeyDown,
  makeBackgroundInert,
  ModalFrame,
  lockDocumentScroll,
  restoreModalFocus,
} from './Modal';

function focusable(name: string) {
  return {
    name,
    hidden: false,
    tabIndex: 0,
    focus: vi.fn(),
    getAttribute: () => null,
    matches: () => false,
    closest: () => null,
  };
}

describe('Modal reutilizável', () => {
  it('expõe semântica modal e nome acessível ligado ao título', () => {
    const markup = renderToStaticMarkup(
      <ModalFrame
        title="Preferências"
        titleId="modal-title"
        close={() => undefined}
        closeOnBackdrop={false}
        dialogRef={() => undefined}
      >
        Conteúdo
      </ModalFrame>,
    );
    expect(markup).toContain('role="dialog"');
    expect(markup).toContain('aria-modal="true"');
    expect(markup).toContain('aria-labelledby="modal-title"');
    expect(markup).toContain('aria-label="Fechar diálogo"');
  });

  it('move o foco inicial para o primeiro controle útil', () => {
    const first = focusable('primeiro');
    const dialog = {
      querySelector: () => null,
      querySelectorAll: () => [first],
      focus: vi.fn(),
    } as unknown as HTMLElement;
    focusInitialElement(dialog);
    expect(first.focus).toHaveBeenCalledOnce();
  });

  it('prioriza o controle marcado para foco inicial sem autofocus nativo', () => {
    const search = focusable('busca');
    const first = focusable('primeiro');
    const dialog = {
      querySelector: () => search,
      querySelectorAll: () => [first, search],
      focus: vi.fn(),
    } as unknown as HTMLElement;
    focusInitialElement(dialog);
    expect(search.focus).toHaveBeenCalledOnce();
    expect(first.focus).not.toHaveBeenCalled();
  });

  it('mantém Tab e Shift+Tab dentro do diálogo e fecha com Escape', () => {
    const first = focusable('primeiro');
    const last = focusable('último');
    const dialog = {
      querySelectorAll: () => [first, last],
      contains: (element: unknown) => element === first || element === last,
    } as unknown as HTMLElement;
    const preventDefault = vi.fn();
    const close = vi.fn();

    handleModalKeyDown(
      dialog,
      { key: 'Tab', shiftKey: true, preventDefault },
      close,
      first as unknown as Element,
    );
    expect(last.focus).toHaveBeenCalledOnce();
    handleModalKeyDown(
      dialog,
      { key: 'Tab', shiftKey: false, preventDefault },
      close,
      last as unknown as Element,
    );
    expect(first.focus).toHaveBeenCalledOnce();
    expect(preventDefault).toHaveBeenCalledTimes(2);

    handleModalKeyDown(
      dialog,
      { key: 'Escape', shiftKey: false, preventDefault },
      close,
      first as unknown as Element,
    );
    expect(close).toHaveBeenCalledOnce();
  });

  it('devolve o foco ao elemento que abriu o diálogo', () => {
    const opener = { isConnected: true, focus: vi.fn() } as unknown as HTMLElement;
    restoreModalFocus(opener);
    expect(opener.focus).toHaveBeenCalledOnce();
  });

  it('fecha no clique do fundo somente quando habilitado', () => {
    const close = vi.fn();
    const backdrop = {} as EventTarget;
    handleBackdropMouseDown({ target: backdrop, currentTarget: backdrop }, false, close);
    expect(close).not.toHaveBeenCalled();
    handleBackdropMouseDown({ target: backdrop, currentTarget: backdrop }, true, close);
    expect(close).toHaveBeenCalledOnce();
    handleBackdropMouseDown({ target: {} as EventTarget, currentTarget: backdrop }, true, close);
    expect(close).toHaveBeenCalledOnce();
  });

  it('torna o fundo inerte sem remover conteúdo ou perder seu estado', () => {
    const draft = { value: 'rascunho ainda ativo' };
    const background = { inert: false, draft };
    const portal = { parentElement: null };
    const fakeBody = {
      children: [background] as unknown[],
      appendChild: (node: { parentElement: unknown }) => {
        fakeBody.children.push(node);
        node.parentElement = fakeBody;
      },
      removeChild: (node: unknown) => {
        fakeBody.children = fakeBody.children.filter((child) => child !== node);
      },
    };
    const body = fakeBody as unknown as HTMLElement;

    const restore = makeBackgroundInert(portal as unknown as HTMLElement, body);
    expect(background.inert).toBe(true);
    expect(fakeBody.children).toContain(background);
    expect(background.draft.value).toBe('rascunho ainda ativo');
    restore();
    expect(background.inert).toBe(false);
    expect(fakeBody.children).not.toContain(portal);
    expect(background.draft.value).toBe('rascunho ainda ativo');
  });

  it('bloqueia a rolagem do fundo e restaura estilos e posição', () => {
    const body = {
      style: {
        position: 'relative',
        top: '3px',
        left: '',
        right: '',
        width: '90%',
        overflow: 'auto',
      },
    } as unknown as HTMLElement;
    const restoreScroll = vi.fn();
    const restore = lockDocumentScroll(body, 180, restoreScroll);
    expect(body.style.position).toBe('fixed');
    expect(body.style.top).toBe('-180px');
    restore();
    expect(body.style.position).toBe('relative');
    expect(body.style.top).toBe('3px');
    expect(body.style.width).toBe('90%');
    expect(body.style.overflow).toBe('auto');
    expect(restoreScroll).toHaveBeenCalledWith(180);
  });

  it('desativa animação do modal com movimento reduzido e modo de desempenho', () => {
    const css = readFileSync(new URL('./styles.css', import.meta.url), 'utf8');
    expect(css).toMatch(/@media \(prefers-reduced-motion: reduce\)\s*\{\s*\.modal-surface/);
    expect(css).toContain("html[data-performance='on'] .modal-surface");
  });
});
