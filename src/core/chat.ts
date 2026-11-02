export type ConversationSummary = {
  id: string;
  title: string;
  preview: string;
  created_at: number;
  provider_id?: string;
  model?: string;
  agent_enabled?: boolean;
};

export function filterConversations<T extends ConversationSummary>(
  conversations: T[],
  search: string,
): T[] {
  const query = search.trim().toLocaleLowerCase('pt-BR');
  if (!query) return conversations;
  return conversations.filter((conversation) =>
    `${conversation.title} ${conversation.preview}`.toLocaleLowerCase('pt-BR').includes(query),
  );
}

export function resolveConversationSelection<T extends ConversationSummary>(
  conversations: T[],
  selectedId: string,
): string | null {
  return conversations.some((conversation) => conversation.id === selectedId) ? selectedId : null;
}

export function canRetryFailedPrompt(
  prompt: string,
  error: string,
  failedWithAgent: boolean,
): boolean {
  return (
    Boolean(prompt.trim()) &&
    !failedWithAgent &&
    /rede|conexão|limitou as solicitações|tempo limite|demorou para responder|serviço do provedor está indisponível/i.test(
      error,
    )
  );
}

export function messageAlignment(role: string): 'right' | 'left' {
  return role === 'user' ? 'right' : 'left';
}

export function presentMessages<T extends { role: string }>(
  messages: T[],
): { message: T; side: 'right' | 'left' }[] {
  return messages.map((message) => ({ message, side: messageAlignment(message.role) }));
}
