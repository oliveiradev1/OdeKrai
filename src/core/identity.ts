export type ConversationNotes = { instructions: string; slang: string };

export type ApprovedProfile = { displayName: string; nickname: string };
export type EditableProfile = ApprovedProfile & { avatarDataUrl: string | null };

export function normalizeProfileData(value: unknown): EditableProfile {
  const profile = value && typeof value === 'object' ? (value as Record<string, unknown>) : {};
  return {
    displayName: typeof profile.displayName === 'string' ? profile.displayName : '',
    nickname: typeof profile.nickname === 'string' ? profile.nickname : '',
    avatarDataUrl: typeof profile.avatarDataUrl === 'string' ? profile.avatarDataUrl : null,
  };
}

export function profileFromApprovedCore(content: string): ApprovedProfile | null {
  const displayName = content.match(/^- Nome: (.*)$/m)?.[1]?.trim() ?? '';
  const savedNickname = content.match(/^- Apelido: (.*)$/m)?.[1]?.trim() ?? '';
  if (!displayName || displayName.startsWith('{{') || !savedNickname) return null;
  return {
    displayName,
    nickname: savedNickname === 'não informado' ? '' : savedNickname,
  };
}

export function mergeApprovedProfile(
  profile: EditableProfile,
  approvedContent: string,
): EditableProfile | null {
  const approved = profileFromApprovedCore(approvedContent);
  if (!approved) return null;
  const current = normalizeProfileData(profile);
  return {
    displayName: current.displayName.trim() ? current.displayName : approved.displayName,
    nickname: current.nickname.trim() ? current.nickname : approved.nickname,
    avatarDataUrl: current.avatarDataUrl,
  };
}

export function sanitizeIdentityName(value: string, maxLength: number): string {
  return value
    .replace(/[\p{C}\r\n]+/gu, ' ')
    .replace(/[^\p{L}\p{N} .'-]/gu, '')
    .replace(/\s+/g, ' ')
    .slice(0, maxLength)
    .trim();
}

export function updateIdentityProfile(
  content: string,
  displayName: string,
  nickname: string,
): string | null {
  const name = sanitizeIdentityName(displayName, 60);
  const alias = sanitizeIdentityName(nickname, 40) || 'não informado';
  if (!name || !/^- Nome: .*$/m.test(content) || !/^- Apelido: .*$/m.test(content)) return null;
  return content
    .replace(/^- Nome: .*$/m, `- Nome: ${name}`)
    .replace(/^- Apelido: .*$/m, `- Apelido: ${alias}`);
}

export function parseConversationNotes(content: string): ConversationNotes {
  const section = (heading: string) =>
    content.match(new RegExp(`^## ${heading}\\n([\\s\\S]*?)(?=\\n## |$)`, 'm'))?.[1]?.trim() ?? '';
  return {
    instructions: section('Instruções pessoais'),
    slang: section('Gírias e expressões'),
  };
}

export function formatConversationNotes({ instructions, slang }: ConversationNotes): string {
  return `# Preferências de conversa\n\n## Instruções pessoais\n${instructions.trim()}\n\n## Gírias e expressões\n${slang.trim()}\n`;
}
