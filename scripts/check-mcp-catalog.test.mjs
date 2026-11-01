import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import { describe, expect, it } from 'vitest';
import { validateMcpCatalog, verifyExamples } from './check-mcp-catalog.mjs';

const catalogPath = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '../src/core/mcp-service-catalog.json',
);
const catalog = JSON.parse(await readFile(catalogPath, 'utf8'));

describe('guarda do catálogo MCP', () => {
  it('aceita o catálogo atual e executa exemplos bons e ruins', () => {
    expect(validateMcpCatalog(catalog)).toEqual([]);
    expect(() => verifyExamples(catalog)).not.toThrow();
  });

  it('reprova endpoints sem HTTPS e hosts de endpoint não confirmados', () => {
    const insecure = structuredClone(catalog);
    insecure[0].endpoint = 'http://api.githubcopilot.com/mcp/';
    expect(validateMcpCatalog(insecure).join('\n')).toContain('use HTTPS');

    const unconfirmedHost = structuredClone(catalog);
    unconfirmedHost[0].endpoint = 'https://mcp.unofficial.example/mcp';
    expect(validateMcpCatalog(unconfirmedHost).join('\n')).toContain(
      'domínio não corresponde ao serviço documentado',
    );
  });

  it('exige o domínio oficial esperado para a página de documentação', () => {
    const changed = structuredClone(catalog);
    changed[1].documentationUrl = 'https://figma-help.example/mcp';
    expect(validateMcpCatalog(changed).join('\n')).toContain('domínio deve ser help.figma.com');
  });

  it('reprova links de terceiros sem justificativa e aceita a marcação explícita', () => {
    const unmarked = structuredClone(catalog);
    unmarked[5].sourceCodeUrl = 'https://community.example/lovable-mcp';
    expect(validateMcpCatalog(unmarked).join('\n')).toContain(
      'endereço de terceiro sem marcação explícita',
    );

    const marked = structuredClone(unmarked);
    marked[5].thirdPartyLinks = [
      { url: marked[5].sourceCodeUrl, reason: 'Referência externa identificada.' },
    ];
    expect(validateMcpCatalog(marked)).toEqual([]);
  });

  it('exige fonte oficial e todos os serviços esperados', () => {
    const noSource = structuredClone(catalog);
    delete noSource[0].documentationUrl;
    expect(validateMcpCatalog(noSource).join('\n')).toContain(
      'link de documentação oficial ausente',
    );
    expect(validateMcpCatalog(catalog.slice(1)).join('\n')).toContain(
      'Serviço esperado ausente do catálogo: GitHub',
    );
  });
});
