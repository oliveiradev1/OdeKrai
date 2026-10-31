import { useEffect, useRef, type FormEvent, type ReactNode, type RefObject } from 'react';

export function shouldSubmitMessageOnEnter(key: string, shiftKey: boolean): boolean {
  return key === 'Enter' && !shiftKey;
}

export function MessageComposer({
  draft,
  setDraft,
  busy,
  canSend = true,
  onSubmit,
  onCancel,
  placeholder = 'Escreva sua mensagem...',
  className = 'composer',
  sendButtonClassName = 'send-button',
  textareaRef: forwardedRef,
  footer,
  children,
}: {
  draft: string;
  setDraft: (value: string) => void;
  busy: boolean;
  canSend?: boolean;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void | Promise<unknown>;
  onCancel?: () => void;
  placeholder?: string;
  className?: string;
  sendButtonClassName?: string;
  textareaRef?: RefObject<HTMLTextAreaElement | null>;
  footer?: (sendButton: ReactNode) => ReactNode;
  children?: ReactNode;
}) {
  const internalRef = useRef<HTMLTextAreaElement>(null);
  const textareaRef = forwardedRef ?? internalRef;
  const submissionLocked = useRef(false);

  useEffect(() => {
    const textarea = textareaRef.current;
    if (!textarea) return;
    textarea.style.height = 'auto';
    textarea.style.height = `${Math.min(textarea.scrollHeight, 120)}px`;
  }, [draft, textareaRef]);

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (submissionLocked.current || busy || !canSend || !draft.trim()) return;
    submissionLocked.current = true;
    try {
      await onSubmit(event);
    } finally {
      submissionLocked.current = false;
    }
  }

  const sendButton = (
    <button
      className={`${sendButtonClassName}${busy ? ' stop-button' : ''}`.trim()}
      type={busy ? 'button' : 'submit'}
      onClick={busy ? onCancel : undefined}
      disabled={!busy && (!canSend || !draft.trim())}
      title={
        !busy && !canSend
          ? 'Conecte um provedor de IA para enviar mensagens.'
          : !busy && !draft.trim()
            ? 'Escreva uma mensagem para habilitar o envio.'
            : undefined
      }
      aria-label={busy ? 'Parar resposta' : 'Enviar mensagem'}
    >
      {busy ? 'Parar' : 'Enviar'}
    </button>
  );

  return (
    <form className={className} onSubmit={(event) => void submit(event)}>
      <textarea
        ref={textareaRef}
        value={draft}
        onChange={(event) => setDraft(event.target.value)}
        onKeyDown={(event) => {
          if (shouldSubmitMessageOnEnter(event.key, event.shiftKey)) {
            event.preventDefault();
            event.currentTarget.form?.requestSubmit();
          }
        }}
        placeholder={placeholder}
        rows={1}
        aria-label="Mensagem"
      />
      {footer ? (
        footer(sendButton)
      ) : (
        <div className="home-composer-footer">
          <span>Enter envia · Shift + Enter quebra linha</span>
          <div className="chat-composer-actions">{sendButton}</div>
        </div>
      )}
      {children}
    </form>
  );
}
