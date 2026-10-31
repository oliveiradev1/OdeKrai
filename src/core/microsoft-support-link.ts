import { openProviderLink, type UrlOpener } from './provider-links';

export const MICROSOFT_SPEECH_SETTINGS_URL =
  'https://support.microsoft.com/pt-br/windows/hardware/input-devices/manage-the-language-and-keyboard-input-layout-settings-in-windows';

export async function openMicrosoftSupportLink(url: string, openUrl: UrlOpener): Promise<void> {
  if (url !== MICROSOFT_SPEECH_SETTINGS_URL) {
    throw new Error('Este link de suporte da Microsoft não está autorizado.');
  }
  await openProviderLink(url, openUrl);
}
