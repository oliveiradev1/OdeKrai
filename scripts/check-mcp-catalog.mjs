import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

// These are the official documentation and service hosts cited by the catalog as of 2026-11-01.
const servicePolicy = {
  GitHub: {
    documentationHost: 'docs.github.com',
    endpointHost: 'api.githubcopilot.com',
    officialHosts: ['docs.github.com', 'api.githubcopilot.com', 'github.com'],
  },
  Figma: {
    documentationHost: 'help.figma.com',
    endpointHost: 'mcp.figma.com',
    officialHosts: ['help.figma.com', 'mcp.figma.com'],
  },
  Gmail: {
    documentationHost: 'developers.google.com',
    endpointHost: 'gmailmcp.googleapis.com',
    officialHosts: ['developers.google.com', 'gmailmcp.googleapis.com', 'www.googleapis.com'],
  },
  'Google Calendar': {
    documentationHost: 'developers.google.com',
    endpointHost: 'calendarmcp.googleapis.com',
    officialHosts: ['developers.google.com', 'calendarmcp.googleapis.com', 'www.googleapis.com'],
  },
  'Google Sheets': {
    documentationHost: 'developers.google.com',
    endpointHost: 'sheetsmcp.googleapis.com',
    officialHosts: ['developers.google.com', 'sheetsmcp.googleapis.com', 'www.googleapis.com'],
  },
  Lovable: {
    documentationHost: 'docs.lovable.dev',
    endpointHost: 'mcp.lovable.dev',
    officialHosts: ['docs.lovable.dev', 'mcp.lovable.dev', 'github.com'],
  },
  Notion: {
    documentationHost: 'developers.notion.com',
    endpointHost: 'mcp.notion.com',
    officialHosts: ['developers.notion.com', 'mcp.notion.com'],
  },
};

function collectUrls(value, location = '$', result = []) {
  if (typeof value === 'string') {
    for (const match of value.matchAll(/https?:\/\/[^\s<>"'`]+/g)) {
      const url = match[0].replace(/[),.;\]}]+$/g, '');
      result.push({ location, url });
    }
  } else if (Array.isArray(value)) {
    value.forEach((item, index) => collectUrls(item, `${location}[${index}]`, result));
  } else if (value && typeof value === 'object') {
    for (const [key, child] of Object.entries(value)) {
      collectUrls(child, `${location}.${key}`, result);
    }
  }
  return result;
}

function parseUrl(raw, location, errors, allowLoopbackHttp = false) {
  let url;
  try {
    url = new URL(raw);
  } catch {
    errors.push(`${location}: URL inválida (${raw}).`);
    return null;
  }

  const isLoopbackCallback =
    allowLoopbackHttp && url.protocol === 'http:' && url.hostname === '127.0.0.1';
  if (url.protocol !== 'https:' && !isLoopbackCallback) {
    errors.push(`${location}: use HTTPS (${raw}).`);
  }
  if (url.username || url.password) errors.push(`${location}: URL não pode conter credenciais.`);
  if ([...url.searchParams.keys()].some((key) => /^utm_/i.test(key))) {
    errors.push(`${location}: remova parâmetros de rastreamento (${raw}).`);
  }
  return url;
}

export function validateMcpCatalog(catalog) {
  const errors = [];
  if (!Array.isArray(catalog)) return ['O catálogo MCP deve ser uma lista.'];

  const seenNames = new Set();
  const seenIds = new Set();
  for (const [index, service] of catalog.entries()) {
    const location = `serviços[${index}]`;
    if (!service || typeof service !== 'object') {
      errors.push(`${location}: item inválido.`);
      continue;
    }

    const policy = servicePolicy[service.name];
    if (!policy) {
      errors.push(
        `${location}: serviço sem política de domínios oficial (${service.name ?? 'sem nome'}).`,
      );
      continue;
    }
    if (seenNames.has(service.name))
      errors.push(`${location}: serviço duplicado (${service.name}).`);
    seenNames.add(service.name);
    if (typeof service.id !== 'string' || !service.id.trim() || seenIds.has(service.id)) {
      errors.push(`${location}: identificador ausente ou duplicado.`);
    }
    seenIds.add(service.id);
    if (typeof service.provider !== 'string' || !service.provider.trim()) {
      errors.push(`${location}: fornecedor ausente.`);
    }
    const allowedConnectionTypes = ['one_click', 'client_id', 'personal_token', 'informational'];
    if (
      !Array.isArray(service.connectionType) ||
      !service.connectionType.length ||
      service.connectionType.some((type) => !allowedConnectionTypes.includes(type))
    ) {
      errors.push(`${location}: tipo de conexão inválido ou ausente.`);
    }
    if (
      !['available', 'in_preview', 'configuration_pending', 'not_verified'].includes(service.status)
    ) {
      errors.push(`${location}: status do catálogo inválido ou ausente.`);
    }
    if (!Array.isArray(service.requirements) || !Array.isArray(service.permissions)) {
      errors.push(`${location}: requisitos e permissões devem ser listas.`);
    }
    if (
      !Array.isArray(service.authenticationMethods) ||
      service.authenticationMethods.length === 0
    ) {
      errors.push(`${location}: método de autenticação ausente.`);
    }

    const thirdPartyLinks = Array.isArray(service.thirdPartyLinks) ? service.thirdPartyLinks : [];
    for (const item of thirdPartyLinks) {
      if (typeof item?.url !== 'string' || typeof item.reason !== 'string' || !item.reason.trim()) {
        errors.push(`${location}: endereço de terceiro precisa de URL e justificativa.`);
      }
    }

    const documentation = parseUrl(service.officialLink, `${location}.officialLink`, errors);
    if (!service.officialLink) errors.push(`${location}: link de documentação oficial ausente.`);
    if (documentation && documentation.hostname !== policy.documentationHost) {
      errors.push(
        `${location}.officialLink: domínio deve ser ${policy.documentationHost}, conforme a fonte oficial citada.`,
      );
    }

    const endpoint = parseUrl(service.endpoint, `${location}.endpoint`, errors);
    if (!service.endpoint) errors.push(`${location}: endpoint ausente.`);
    if (endpoint && endpoint.hostname !== policy.endpointHost) {
      errors.push(
        `${location}.endpoint: domínio não corresponde ao serviço documentado (${policy.endpointHost}).`,
      );
    }

    for (const { location: urlLocation, url: raw } of collectUrls(service, location)) {
      const parsed = parseUrl(raw, urlLocation, errors, true);
      if (!parsed || (parsed.protocol === 'http:' && parsed.hostname === '127.0.0.1')) continue;
      const markedThirdParty = thirdPartyLinks.some(
        (item) => item.url === raw && typeof item.reason === 'string' && item.reason.trim(),
      );
      if (!policy.officialHosts.includes(parsed.hostname) && !markedThirdParty) {
        errors.push(`${urlLocation}: endereço de terceiro sem marcação explícita (${raw}).`);
      }
    }
  }

  for (const serviceName of Object.keys(servicePolicy)) {
    if (!seenNames.has(serviceName))
      errors.push(`Serviço esperado ausente do catálogo: ${serviceName}.`);
  }

  return errors;
}

export function verifyExamples(catalog) {
  assert.deepEqual(validateMcpCatalog(catalog), []);
  const insecure = structuredClone(catalog);
  insecure[0].endpoint = insecure[0].endpoint.replace('https://', 'http://');
  assert.ok(validateMcpCatalog(insecure).some((error) => error.includes('use HTTPS')));

  const wrongDomain = structuredClone(catalog);
  wrongDomain[0].endpoint = 'https://unofficial.example/mcp';
  assert.ok(
    validateMcpCatalog(wrongDomain).some((error) => error.includes('domínio não corresponde')),
  );

  const unmarkedThirdParty = structuredClone(catalog);
  unmarkedThirdParty[5].sourceCodeUrl = 'https://unofficial.example/lovable-mcp';
  assert.ok(
    validateMcpCatalog(unmarkedThirdParty).some((error) =>
      error.includes('sem marcação explícita'),
    ),
  );

  const markedThirdParty = structuredClone(catalog);
  markedThirdParty[5].sourceCodeUrl = 'https://unofficial.example/lovable-mcp';
  markedThirdParty[5].thirdPartyLinks = [
    { url: markedThirdParty[5].sourceCodeUrl, reason: 'Repositório externo de referência.' },
  ];
  assert.deepEqual(validateMcpCatalog(markedThirdParty), []);
}

async function main() {
  const catalogPath = path.join(root, 'src-tauri/resources/public-mcp-catalog.json');
  const catalog = JSON.parse(await readFile(catalogPath, 'utf8'));
  verifyExamples(catalog);
  const errors = validateMcpCatalog(catalog);
  if (errors.length) {
    console.error('Catálogo MCP inválido:');
    for (const error of errors) console.error(`- ${error}`);
    process.exitCode = 1;
    return;
  }
  console.log(
    `Catálogo MCP validado: ${catalog.length} serviços, HTTPS e domínios oficiais conferidos.`,
  );
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  await main();
}
