import catalog from './mcp-service-catalog.json';

export type PublicMcpCatalogEntry = (typeof catalog)[number];
export type PublicMcpStatus = 'available' | 'configured' | 'connected' | 'not_available';
export type OAuthRegistrationOptions = { dynamicRegistration: boolean };

export function canConnectWithOAuth(
  options: OAuthRegistrationOptions | null,
  clientId: string,
  busy = false,
): boolean {
  return !busy && options !== null && (options.dynamicRegistration || Boolean(clientId.trim()));
}

export function oauthClientIdHelp(
  service: PublicMcpCatalogEntry,
  options: OAuthRegistrationOptions | null,
): string | null {
  if (!options || options.dynamicRegistration) return null;
  return `Este servidor exige um identificador de cliente. Veja a documentação do serviço (${service.documentationUrl}) para criá-lo.`;
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
    return configuredServer.enabled && configuredServer.status === 'Conectado'
      ? 'connected'
      : 'configured';
  }
  return service.status === 'available_to_configure' ? 'available' : 'not_available';
}

export const publicMcpStatusLabels: Record<PublicMcpStatus, string> = {
  available: 'Disponível para configurar',
  configured: 'Configurado',
  connected: 'Conectado',
  not_available: 'Não disponível',
};
