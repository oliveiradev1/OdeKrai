import cloudLinks from './cloud-links.json';
import { openProviderLink, type UrlOpener } from './provider-links';

export const CLOUD_LINK_CATEGORIES = cloudLinks.categories;
export const CLOUD_LINK_DOMAINS = cloudLinks.allowedDomains;

export async function openCloudLink(url: string, openValidatedUrl: UrlOpener): Promise<void> {
  let parsed: URL;
  try {
    parsed = new URL(url);
  } catch {
    throw new Error('Endereço inválido.');
  }
  if (parsed.protocol !== 'https:') {
    throw new Error('Os links da Nuvem precisam usar HTTPS.');
  }
  if (!CLOUD_LINK_DOMAINS.includes(parsed.hostname)) {
    throw new Error('Este domínio não está na lista autorizada de links da Nuvem.');
  }
  await openProviderLink(parsed.href, openValidatedUrl);
}
