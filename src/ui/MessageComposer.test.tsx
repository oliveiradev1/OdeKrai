import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { MessageComposer, shouldSubmitMessageOnEnter } from './MessageComposer';

describe('MessageComposer', () => {
  it('preserva Enter, Shift+Enter e o nome acessível do campo compartilhado', () => {
    const markup = renderToStaticMarkup(
      <MessageComposer
        draft=""
        setDraft={() => undefined}
        busy={false}
        onSubmit={() => undefined}
      />,
    );
    expect(shouldSubmitMessageOnEnter('Enter', false)).toBe(true);
    expect(shouldSubmitMessageOnEnter('Enter', true)).toBe(false);
    expect(shouldSubmitMessageOnEnter('Escape', false)).toBe(false);
    expect(markup).toContain('aria-label="Mensagem"');
    expect(markup).toContain('aria-label="Enviar mensagem"');
    expect(markup).toContain('disabled=""');
  });

  it('expõe o estado de envio e o controle para parar a resposta', () => {
    const markup = renderToStaticMarkup(
      <MessageComposer
        draft="Olá"
        setDraft={() => undefined}
        busy
        onSubmit={() => undefined}
        onCancel={() => undefined}
      />,
    );
    expect(markup).toContain('class="send-button stop-button"');
    expect(markup).toContain('aria-label="Parar resposta"');
    expect(markup).toContain('>Parar</button>');
  });
});
