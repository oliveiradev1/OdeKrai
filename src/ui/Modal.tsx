import { useId, useLayoutEffect, useRef, useState, type RefCallback, type ReactNode } from 'react';
import { createPortal } from 'react-dom';

const FOCUSABLE_SELECTOR =
  'a[href], area[href], button, input, select, textarea, iframe, object, embed, summary, audio[controls], video[controls], [contenteditable="true"], [tabindex]';

type ModalKeyEvent = Pick<KeyboardEvent, 'key' | 'shiftKey' | 'preventDefault'>;

export function focusableElements(dialog: HTMLElement): HTMLElement[] {
  return Array.from(dialog.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR)).filter(
    (element) =>
      element.tabIndex >= 0 &&
      !element.matches(':disabled') &&
      !element.closest('[hidden], [inert], [aria-hidden="true"]'),
  );
}

export function focusInitialElement(dialog: HTMLElement): void {
  const preferred = dialog.querySelector<HTMLElement>('[autofocus], [data-autofocus]');
  const focusable = focusableElements(dialog);
  (preferred && focusable.includes(preferred) ? preferred : (focusable[0] ?? dialog)).focus();
}

export function handleModalKeyDown(
  dialog: HTMLElement,
  event: ModalKeyEvent,
  onClose: () => void,
  activeElement: Element | null = document.activeElement,
): void {
  if (event.key === 'Escape') {
    event.preventDefault();
    onClose();
    return;
  }
  if (event.key !== 'Tab') return;

  const elements = focusableElements(dialog);
  if (elements.length === 0) {
    event.preventDefault();
    dialog.focus();
    return;
  }
  const first = elements[0];
  const last = elements[elements.length - 1];
  if (event.shiftKey && (activeElement === first || !dialog.contains(activeElement))) {
    event.preventDefault();
    last.focus();
  } else if (!event.shiftKey && (activeElement === last || !dialog.contains(activeElement))) {
    event.preventDefault();
    first.focus();
  }
}

export function makeBackgroundInert(
  portal: HTMLElement,
  body: HTMLElement = document.body,
): () => void {
  if (portal.parentElement !== body) body.appendChild(portal);
  const previous = Array.from(body.children)
    .filter((element) => element !== portal)
    .map((element) => {
      const node = element as HTMLElement;
      const wasInert = node.inert;
      node.inert = true;
      return { node, wasInert };
    });
  return () => {
    previous.forEach(({ node, wasInert }) => {
      node.inert = wasInert;
    });
    if (portal.parentElement === body) body.removeChild(portal);
  };
}

type ScrollStyle = 'position' | 'top' | 'left' | 'right' | 'width' | 'overflow';

export function lockDocumentScroll(
  body: HTMLElement,
  scrollY: number,
  restoreScroll: (y: number) => void,
): () => void {
  const properties: ScrollStyle[] = ['position', 'top', 'left', 'right', 'width', 'overflow'];
  const previous = new Map(properties.map((property) => [property, body.style[property]]));
  body.style.position = 'fixed';
  body.style.top = `${-scrollY}px`;
  body.style.left = '0';
  body.style.right = '0';
  body.style.width = '100%';
  body.style.overflow = 'hidden';
  return () => {
    properties.forEach((property) => {
      body.style[property] = previous.get(property) ?? '';
    });
    restoreScroll(scrollY);
  };
}

export function restoreModalFocus(element: HTMLElement | null): void {
  if (element?.isConnected) element.focus();
}

export function handleBackdropMouseDown(
  event: Pick<MouseEvent, 'target' | 'currentTarget'>,
  closeOnBackdrop: boolean,
  onClose: () => void,
): void {
  if (closeOnBackdrop && event.target === event.currentTarget) onClose();
}

export function ModalFrame({
  title,
  titleId,
  close,
  closeOnBackdrop,
  dialogRef,
  children,
}: {
  title: string;
  titleId: string;
  close: () => void;
  closeOnBackdrop: boolean;
  dialogRef: RefCallback<HTMLElement>;
  children: ReactNode;
}) {
  return (
    <div
      className="modal-backdrop"
      onMouseDown={(event) => {
        handleBackdropMouseDown(event, closeOnBackdrop, close);
      }}
    >
      <section
        className="modal-surface"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        tabIndex={-1}
        ref={dialogRef}
      >
        <header className="modal-header">
          <h2 id={titleId}>{title}</h2>
          <button className="modal-close" type="button" aria-label="Fechar diálogo" onClick={close}>
            ×
          </button>
        </header>
        <div className="modal-body">{children}</div>
      </section>
    </div>
  );
}

export function Modal({
  open,
  title,
  onClose,
  closeOnBackdrop = false,
  children,
}: {
  open: boolean;
  title: string;
  onClose: () => void;
  closeOnBackdrop?: boolean;
  children: ReactNode;
}) {
  const titleId = useId();
  const [portal] = useState<HTMLElement | null>(() =>
    typeof document === 'undefined' ? null : document.createElement('div'),
  );
  const [dialog, setDialog] = useState<HTMLElement | null>(null);
  const closeRef = useRef(onClose);

  useLayoutEffect(() => {
    closeRef.current = onClose;
  }, [onClose]);

  useLayoutEffect(() => {
    if (!open || !portal || !dialog) return;
    const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const restoreInert = makeBackgroundInert(portal);
    const restoreScroll = lockDocumentScroll(document.body, window.scrollY, (y) =>
      window.scrollTo(0, y),
    );
    focusInitialElement(dialog);

    const handleKeyDown = (event: KeyboardEvent) =>
      handleModalKeyDown(dialog, event, () => closeRef.current());
    document.addEventListener('keydown', handleKeyDown, true);
    return () => {
      document.removeEventListener('keydown', handleKeyDown, true);
      restoreInert();
      restoreScroll();
      restoreModalFocus(opener);
    };
  }, [open, portal, dialog]);

  if (!open || !portal) return null;
  return createPortal(
    <ModalFrame
      title={title}
      titleId={titleId}
      close={onClose}
      closeOnBackdrop={closeOnBackdrop}
      dialogRef={setDialog}
    >
      {children}
    </ModalFrame>,
    portal,
  );
}
