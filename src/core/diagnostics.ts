export type DiagnosticItem = {
  key: string;
  command: string;
  message: string;
  details: string;
  count: number;
};

function redactMarkerValues(input: string, marker: RegExp): string {
  return input.replace(marker, (value) => {
    const prefix = value.match(
      /^(?:Bearer\s+|sk-|pplx-|api[_-]?key\s*[:=]\s*|token\s*[:=]\s*|client_secret\s*[:=]\s*|password\s*[:=]\s*|authorization\s*:\s*)/i,
    )?.[0];
    return `${prefix ?? ''}[credencial removida]`;
  });
}

/** Redação equivalente à do diagnostic.log, com proteção adicional para caminhos locais. */
export function sanitizeDiagnosticDetails(error: unknown): string {
  const source =
    error instanceof Error ? (error.stack ?? `${error.name}: ${error.message}`) : String(error);
  let details = [...source]
    .slice(0, 512)
    .map((character) => (/[\r\n\t\x00-\x1f\x7f]/.test(character) ? '_' : character))
    .join('');

  details = details
    .replace(/[A-Z]:\\[^\r\n"'<>|?*]*/gi, '[caminho local]')
    .replace(/\\\\[^\\\s]+\\[^\r\n"'<>|?*]*/g, '[caminho local]')
    .replace(/(?:\/Users\/[^/\s]+|\/home\/[^/\s]+|\/root)(?:\/[^\s"'<>]*)?/g, '[caminho local]')
    .replace(/(?:%USERPROFILE%|\$HOME)(?:[\\/][^\s"'<>]*)?/gi, '[caminho local]')
    .replace(
      /\b(messages?|conversation|transcript|prompt|content|request[_-]?body|user[_-]?text)\s*[:=]\s*[^\r\n]*/gi,
      '$1=[conteúdo omitido]',
    );

  details = redactMarkerValues(
    details,
    /(?:Bearer\s+|sk-|pplx-|api[_-]?key\s*[:=]\s*|token\s*[:=]\s*|client_secret\s*[:=]\s*|password\s*[:=]\s*|authorization\s*:\s*)[^\s,;"'<>)}]+/gi,
  );
  details = details.replace(/[\w.+-]+@[\w.-]+\.[A-Za-z]{2,}/g, '[email removido]');
  return details.slice(0, 512);
}

export function diagnosticUserMessage(command: string, message: string, details: string): string {
  if (command === 'frontend.unhandledrejection') {
    if (/\b(?:acl|permission|permiss[aã]o|not allowed)\b/i.test(details)) {
      return 'Uma ação falhou: permissão do sistema não concedida para esta janela.';
    }
    if (/\b(?:timeout|timed out|tempo limite)\b/i.test(details)) {
      return 'Uma ação falhou: o tempo de resposta foi excedido.';
    }
    if (/\b(?:network|fetch|conex[aã]o|rede)\b/i.test(details)) {
      return 'Uma ação falhou: não foi possível acessar o serviço.';
    }
    return 'Uma ação falhou. Consulte os detalhes técnicos.';
  }
  if (command === 'frontend.window.onerror') {
    return 'A interface encontrou um erro. Consulte os detalhes técnicos.';
  }
  return sanitizeDiagnosticDetails(message);
}

function normalizedCause(details: string): string {
  return details
    .replace(/refer[eê]ncia\s*[:#]?\s*[0-9a-f-]{16,}/gi, '')
    .replace(/\b[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}\b/gi, '')
    .replace(/\s+/g, ' ')
    .trim()
    .toLocaleLowerCase('pt-BR');
}

export function upsertDiagnostic(
  items: DiagnosticItem[],
  command: string,
  message: string,
  rawDetails = '',
): DiagnosticItem[] {
  if (!message) return items.filter((item) => item.command !== command);

  const details = rawDetails ? sanitizeDiagnosticDetails(rawDetails) : '';
  const userMessage = diagnosticUserMessage(command, message, details);
  const key = `${command}\u241f${userMessage}\u241f${normalizedCause(details)}`;
  const existing = items.find((item) => item.key === key);
  if (existing) {
    return items.map((item) =>
      item.key === key
        ? { ...item, count: item.count + 1, details: details || item.details }
        : item,
    );
  }
  return [...items.slice(-7), { key, command, message: userMessage, details, count: 1 }];
}

export function removeDiagnostic(items: DiagnosticItem[], key: string): DiagnosticItem[] {
  return items.filter((item) => item.key !== key);
}
