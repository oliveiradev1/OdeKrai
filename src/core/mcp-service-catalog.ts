import catalog from './mcp-service-catalog.json';

export type PublicMcpCatalogEntry = (typeof catalog)[number];
export type PublicMcpStatus = 'available' | 'configured' | 'connecting' | 'connected' | 'error';
export type OAuthRegistrationOptions = { dynamicRegistration: boolean };
export type PublicOAuthClientSettings = {
  service: string;
  clientId: string;
  guideUrl: string;
  dynamicRegistration: boolean;
  clientSecretRequired: boolean;
  clientSecretSaved: boolean;
};

export function publicOAuthServiceKey(service: PublicMcpCatalogEntry): string {
  if (service.supplier === 'Google Workspace') return 'google';
  return service.name.toLocaleLowerCase('en-US');
}

export function canConnectWithOAuth(
  options: OAuthRegistrationOptions | null,
  clientId: string,
  busy = false,
): boolean {
  return !busy && options !== null && (options.dynamicRegistration || Boolean(clientId.trim()));
}

export function canConnectToPublicOAuth(
  settings: PublicOAuthClientSettings | null,
  options: OAuthRegistrationOptions | null,
  busy = false,
): boolean {
  if (busy || !settings || !options) return false;
  if (!settings.clientId.trim()) {
    return settings.dynamicRegistration && options.dynamicRegistration;
  }
  return !settings.clientSecretRequired || settings.clientSecretSaved;
}

export function oauthClientIdHelp(
  service: PublicMcpCatalogEntry,
  options: OAuthRegistrationOptions | null,
): string | null {
  if (!options || options.dynamicRegistration) return null;
  return `Este serviço ainda não foi configurado para o OdeKrai. Veja o guia (${service.documentationUrl}).`;
}

export type PublicMcpServer = {
  name?: string;
  endpoint: string | null;
  enabled: boolean;
  status: string;
};

export const publicMcpCatalog: readonly PublicMcpCatalogEntry[] = catalog;

export function getPublicMcpStatus(
  service: PublicMcpCatalogEntry,
  servers: readonly PublicMcpServer[],
): PublicMcpStatus {
  const configuredServer = servers.find(
    (server) => server.endpoint === service.endpoint || server.name === service.name,
  );
  if (configuredServer) {
    if (configuredServer.status === 'Conectado' && configuredServer.enabled) return 'connected';
    if (/connecting|conectando/i.test(configuredServer.status)) return 'connecting';
    if (/error|erro|falha/i.test(configuredServer.status)) return 'error';
    return 'configured';
  }
  return service.status === 'available_to_configure' ? 'available' : 'configured';
}

export const publicMcpStatusLabels: Record<PublicMcpStatus, string> = {
  available: 'Disponível',
  configured: 'Configure o serviço',
  connecting: 'Conectando',
  connected: 'Conectado',
  error: 'Erro',
};

export function publicMcpErrorMessage(cause: unknown): string {
  const detail = String(cause).toLocaleLowerCase('pt-BR');
  if (
    /401|403|invalid.?grant|unauthorized|forbidden|token.*recus|authorization denied/.test(detail)
  ) {
    return 'O serviço recusou a autorização';
  }
  if (/timeout|timed out|tempo limite|expirou|expired|cancelad/.test(detail)) {
    return 'A conexão expirou, tente de novo';
  }
  if (/insufficient|permission|permissão|scope|escopo/.test(detail)) {
    return 'Permissão insuficiente';
  }
  return 'Não foi possível conectar. Confira a configuração e tente de novo.';
}
