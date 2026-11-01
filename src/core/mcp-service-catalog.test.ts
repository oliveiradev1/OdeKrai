import { describe, expect, it } from 'vitest';
import catalog from './mcp-service-catalog.json';

describe('catálogo público de servidores MCP', () => {
  it('contém somente os sete serviços pedidos, com fontes e dados de transporte/autenticação', () => {
    expect(catalog.map(({ name }) => name)).toEqual([
      'GitHub',
      'Figma',
      'Gmail',
      'Google Calendar',
      'Google Sheets',
      'Lovable',
      'Notion',
    ]);

    for (const service of catalog) {
      expect(service.supplier).toBeTruthy();
      expect(service.documentationUrl).toMatch(/^https:\/\//);
      expect(service.endpoint).toMatch(/^https:\/\//);
      expect(service.transport).toBeTruthy();
      expect(service.authentication).toBeTruthy();
      expect(['available_to_configure', 'not_available']).toContain(service.status);
    }
  });

  it('não apresenta como disponível no OdeKrai serviços com requisito incompatível documentado', () => {
    const unavailable = catalog
      .filter(({ status }) => status === 'not_available')
      .map(({ name }) => name);

    expect(unavailable).toEqual(['Figma', 'Gmail', 'Google Calendar', 'Google Sheets', 'Lovable']);
  });
});
