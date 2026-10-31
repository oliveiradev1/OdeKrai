import type { HomeSuggestion } from './home';

export type IslandQuickRequest = {
  requestId: string;
  conversationId: string;
  text: string;
};

export type IslandKeyboardFocusEvent = 'request' | 'blur' | 'send' | 'escape';
export type IslandReviewDestination = 'approval-window' | 'calendar' | 'conversation' | 'none';
export type IslandReviewState =
  'awaiting_approval' | 'notification' | 'error' | 'completed' | 'paused';

export function isIslandReviewState(state: string): state is IslandReviewState {
  return ['awaiting_approval', 'notification', 'error', 'completed', 'paused'].includes(state);
}

export function islandKeyboardFocus(focused: boolean, event: IslandKeyboardFocusEvent): boolean {
  if (event === 'request') return true;
  if (event === 'blur' || event === 'send' || event === 'escape') return false;
  return focused;
}

export function createIslandQuickRequest(
  rawText: string,
  createId: () => string = () => crypto.randomUUID(),
): IslandQuickRequest | null {
  const text = rawText.trim();
  if (!text) return null;
  return { requestId: createId(), conversationId: createId(), text };
}

export function islandSuggestions(suggestions: HomeSuggestion[]): HomeSuggestion[] {
  return suggestions.slice(0, 3);
}

export function islandReviewDestination(
  state: IslandReviewState,
  hasConversation: boolean,
): IslandReviewDestination {
  if (state === 'awaiting_approval') return 'approval-window';
  if (state === 'notification') return 'calendar';
  return hasConversation ? 'conversation' : 'none';
}

export function canSubmitIslandQuickCommand(text: string, busy: boolean): boolean {
  return !busy && Boolean(text.trim());
}

export function summarizeIslandResponse(text: string, maxLength = 280): string {
  const normalized = text.replace(/\s+/g, ' ').trim();
  if (normalized.length <= maxLength) return normalized;
  return `${normalized.slice(0, Math.max(0, maxLength - 1)).trimEnd()}…`;
}
