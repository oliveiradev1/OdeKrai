import { describe, expect, it, vi } from 'vitest';
import { MICROSOFT_SPEECH_SETTINGS_URL, openMicrosoftSupportLink } from './microsoft-support-link';

describe('openMicrosoftSupportLink', () => {
  it('abre pelo opener validado o link exato de suporte', async () => {
    const opener = vi.fn(async () => {});

    await openMicrosoftSupportLink(MICROSOFT_SPEECH_SETTINGS_URL, opener);

    expect(opener).toHaveBeenCalledWith(MICROSOFT_SPEECH_SETTINGS_URL);
  });

  it('recusa outro domínio e não chama o opener', async () => {
    const opener = vi.fn(async () => {});

    await expect(openMicrosoftSupportLink('https://example.com/', opener)).rejects.toThrow(
      'não está autorizado',
    );
    expect(opener).not.toHaveBeenCalled();
  });
});
