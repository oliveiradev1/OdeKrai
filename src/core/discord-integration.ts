export type SkillCommand = (command: string, args?: Record<string, unknown>) => Promise<unknown>;

export async function testDiscordConnection(
  url: string,
  authorized: boolean,
  command: SkillCommand,
): Promise<void> {
  if (!authorized) throw new Error('Confirme a autorização antes de testar a conexão.');
  await command('discord_test_connection', { url });
}

export async function saveDiscordIntegration(
  name: string,
  url: string,
  authorized: boolean,
  connectionTested: boolean,
  command: SkillCommand,
): Promise<void> {
  if (!authorized) throw new Error('Confirme a autorização antes de salvar a integração.');
  if (!connectionTested) throw new Error('Teste a conexão antes de salvar a integração.');
  await command('discord_save', { name, url });
}

export async function revokeDiscordIntegration(
  webhookId: string,
  command: SkillCommand,
): Promise<void> {
  await command('discord_remove', { webhookId });
}
