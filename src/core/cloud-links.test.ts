import { describe, expect, it, vi } from 'vitest';
import { CLOUD_LINK_CATEGORIES, CLOUD_LINK_DOMAINS, openCloudLink } from './cloud-links';

describe('links da Nuvem', () => {
  it('abre links HTTPS listados pelo caminho validado com o opener injetado', async () => {
    const opener = vi.fn(async (_url: string) => undefined);
    const shardCloud = CLOUD_LINK_CATEGORIES.flatMap((category) => category.services).find(
      (service) => service.name === 'ShardCloud',
    );

    expect(shardCloud).toBeDefined();
    const url = shardCloud?.url ?? '';
    expect(url).toBe('https://shardcloud.app/pt-br/dash/vps');
    await openCloudLink(url, opener);

    expect(opener).toHaveBeenCalledWith('https://shardcloud.app/pt-br/dash/vps');
  });

  it('recusa protocolo HTTPS ausente e domínio que não consta da lista sem chamar o opener', async () => {
    const opener = vi.fn(async (_url: string) => undefined);

    await expect(openCloudLink('http://magalu.cloud/', opener)).rejects.toThrow('HTTPS');
    await expect(openCloudLink('https://magalu.cloud.evil.example/', opener)).rejects.toThrow(
      'não está na lista autorizada',
    );
    await expect(openCloudLink('https://example.com/', opener)).rejects.toThrow(
      'não está na lista autorizada',
    );
    expect(opener).not.toHaveBeenCalled();
  });

  it('mantém cada serviço HTTPS dentro da lista única de domínios autorizados', () => {
    const services = CLOUD_LINK_CATEGORIES.flatMap((category) => category.services);
    expect(services).toHaveLength(7);
    for (const service of services) {
      const url = new URL(service.url);
      expect(url.protocol).toBe('https:');
      expect(CLOUD_LINK_DOMAINS).toContain(url.hostname);
    }
  });
});
