import { describe, expect, it, vi } from 'vitest';
import {
  revokeDiscordIntegration,
  saveDiscordIntegration,
  testDiscordConnection,
} from './discord-integration';

describe('configuração da integração Discord', () => {
  it('testa com autorização antes de salvar e nunca salva se o teste falhar', async () => {
    const command = vi.fn(async (_command: string, _args?: Record<string, unknown>) => undefined);
    await expect(testDiscordConnection('url-secreta', false, command)).rejects.toThrow(
      'Confirme a autorização',
    );
    expect(command).not.toHaveBeenCalled();
    await testDiscordConnection('url-secreta', true, command);
    await saveDiscordIntegration('Avisos', 'url-secreta', true, true, command);
    expect(command.mock.calls.map(([name]) => name)).toEqual([
      'discord_test_connection',
      'discord_save',
    ]);
  });

  it('exige teste bem-sucedido antes de salvar a credencial', async () => {
    const command = vi.fn(async (_command: string, _args?: Record<string, unknown>) => undefined);
    await expect(
      saveDiscordIntegration('Avisos', 'url-secreta', true, false, command),
    ).rejects.toThrow('Teste a conexão');
    expect(command).not.toHaveBeenCalled();
  });

  it('encaminha revogação pelo identificador local; Rust exige confirmação nativa', async () => {
    const command = vi.fn(async (_command: string, _args?: Record<string, unknown>) => undefined);
    await revokeDiscordIntegration('discord-123', command);
    expect(command).toHaveBeenCalledWith('discord_remove', { webhookId: 'discord-123' });
  });
});
