import catalog from './mcp-service-catalog.json';

export type PublicMcpCatalogEntry = (typeof catalog)[number];
export type PublicMcpStatus = 'available' | 'configured' | 'connected' | 'not_available';

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
