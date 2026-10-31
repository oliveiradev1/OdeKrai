export type BuildInfo = {
  version: string;
  commit: string;
  build_date: string;
  webview_version: string;
  windows_version: string;
};

function stringOrEmpty(value: unknown): string {
  return typeof value === 'string' ? value : '';
}

export function normalizeBuildInfo(value: unknown): BuildInfo | null {
  if (!value || typeof value !== 'object') return null;
  const info = value as Record<string, unknown>;
  return {
    version: stringOrEmpty(info.version),
    commit: stringOrEmpty(info.commit),
    build_date: stringOrEmpty(info.build_date),
    webview_version: stringOrEmpty(info.webview_version),
    windows_version: stringOrEmpty(info.windows_version),
  };
}
