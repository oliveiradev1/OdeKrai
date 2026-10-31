import { describe, expect, it } from 'vitest';
import {
  createIslandQuickRequest,
  canSubmitIslandQuickCommand,
  islandKeyboardFocus,
  isIslandReviewState,
  islandReviewDestination,
  islandSuggestions,
  summarizeIslandResponse,
} from './island-quick';
import type { HomeSuggestion } from './home';

const suggestions = Array.from({ length: 5 }, (_, index): HomeSuggestion => ({
  id: `s${index}`,
  label: `Sugestão ${index}`,
  prompt: `Texto ${index}`,
  requirements: [],
}));

describe('comando rápido da ilha', () => {
  it('só habilita foco após pedido e o libera ao desfocar, enviar ou pressionar Esc', () => {
    expect(islandKeyboardFocus(false, 'request')).toBe(true);
    expect(islandKeyboardFocus(true, 'blur')).toBe(false);
    expect(islandKeyboardFocus(true, 'send')).toBe(false);
    expect(islandKeyboardFocus(true, 'escape')).toBe(false);
  });

  it('cria uma conversa rápida com texto revisado e não cria para texto vazio', () => {
    let sequence = 0;
    const createId = () => `id-${++sequence}`;
    expect(createIslandQuickRequest('  Me ajude  ', createId)).toEqual({
      requestId: 'id-1',
      conversationId: 'id-2',
      text: 'Me ajude',
    });
    expect(createIslandQuickRequest('  ', createId)).toBeNull();
  });

  it('limita sugestões reais a três e resume respostas longas', () => {
    expect(islandSuggestions(suggestions).map(({ id }) => id)).toEqual(['s0', 's1', 's2']);
    expect(summarizeIslandResponse(' resposta '.repeat(50), 24)).toHaveLength(24);
  });

  it('não oferece decisão de aprovação na ilha e encaminha Revisar ao destino correspondente', () => {
    expect(islandReviewDestination('awaiting_approval', true)).toBe('approval-window');
    expect(islandReviewDestination('notification', false)).toBe('calendar');
    expect(islandReviewDestination('error', true)).toBe('conversation');
    expect(islandReviewDestination('error', false)).toBe('none');
    expect(isIslandReviewState('thinking')).toBe(false);
    expect(isIslandReviewState('awaiting_approval')).toBe(true);
  });

  it('bloqueia envio vazio e duplicado', () => {
    expect(canSubmitIslandQuickCommand('   ', false)).toBe(false);
    expect(canSubmitIslandQuickCommand('pergunta', true)).toBe(false);
    expect(canSubmitIslandQuickCommand(' pergunta ', false)).toBe(true);
  });
});
