import { afterEach, describe, expect, it, vi } from 'vitest';

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock, isTauri: () => true }));

import { sanitizeDiagnosticDetails, startupInvoke } from './hooks/useAppEffects';
import {
  diagnosticUserMessage,
  removeDiagnostic,
  sanitizeDiagnosticDetails as sanitizeDetails,
  upsertDiagnostic,
} from '../core/diagnostics';

afterEach(() => vi.unstubAllGlobals());

describe('diagnósticos da interface', () => {
  it('sanitiza stack traces sem descartar a origem do erro', () => {
    const error = new Error('Falha api_key=segredo-chave em ana@example.com');
    error.stack = `Error: ${error.message}\n    at run (C:\\Users\\Ana\\app.js:4:2)`;
    const details = sanitizeDiagnosticDetails(error);
    expect(details).toContain('Error:');
    expect(details).toContain('[caminho local]');
    expect(details).not.toContain('segredo-chave');
    expect(details).not.toContain('ana@example.com');
  });

  it('agrupa rejeições iguais mesmo quando cada evento tem uma referência diferente', () => {
    const first = upsertDiagnostic(
      [],
      'frontend.unhandledrejection',
      'Uma ação falhou.',
      'Referência 123e4567-e89b-12d3-a456-426614174000: Command plugin:dialog|message not allowed by ACL',
    );
    const second = upsertDiagnostic(
      first,
      'frontend.unhandledrejection',
      'Uma ação falhou.',
      'Referência 223e4567-e89b-12d3-a456-426614174000: Command plugin:dialog|message not allowed by ACL',
    );

    expect(second).toHaveLength(1);
    expect(second[0].count).toBe(2);
    expect(second[0].message).toBe(
      'Uma ação falhou: permissão do sistema não concedida para esta janela.',
    );
    expect(second[0].message).not.toContain('frontend.unhandledrejection');
    expect(second[0].message).not.toContain('123e4567');
  });

  it('traduz causas técnicas conhecidas em mensagens curtas em português', () => {
    expect(diagnosticUserMessage('frontend.unhandledrejection', '', 'network request failed')).toBe(
      'Uma ação falhou: não foi possível acessar o serviço.',
    );
    expect(diagnosticUserMessage('frontend.unhandledrejection', '', 'request timed out')).toBe(
      'Uma ação falhou: o tempo de resposta foi excedido.',
    );
    expect(diagnosticUserMessage('frontend.window.onerror', 'erro', '')).toBe(
      'A interface encontrou um erro. Consulte os detalhes técnicos.',
    );
  });

  it('remove o aviso quando a mesma leitura tem sucesso e permite fechar um aviso individual', () => {
    const items = upsertDiagnostic(
      [],
      'get_profile',
      'Não foi possível carregar o perfil local.',
      'database busy',
    );
    const withAnother = upsertDiagnostic(
      items,
      'get_permission_settings',
      'Não foi possível carregar as permissões.',
      'database busy',
    );
    const recovered = upsertDiagnostic(withAnother, 'get_profile', '');

    expect(recovered.map((item) => item.command)).toEqual(['get_permission_settings']);
    expect(removeDiagnostic(recovered, recovered[0].key)).toEqual([]);
  });

  it('saneia conteúdo de conversa, credenciais, e-mails e caminhos completos', () => {
    const details = sanitizeDetails(
      'Error: api_key=segredo password=senha ana@example.com conversation=texto privado',
    );
    const pathDetails = sanitizeDetails(
      'Error: falha em C:\\Users\\Ana Silva\\AppData\\Local\\OdeKrai\\app.js:3:4',
    );

    expect(details).not.toContain('segredo');
    expect(details).not.toContain('senha');
    expect(details).not.toContain('ana@example.com');
    expect(details).not.toContain('texto privado');
    expect(pathDetails).not.toContain('C:\\Users\\Ana Silva');
    expect(pathDetails).toContain('[caminho local]');
  });

  it('converte rejeição IPC em diagnóstico da chamada sem deixá-la desatendida', async () => {
    vi.stubGlobal('window', {
      setTimeout: globalThis.setTimeout,
      clearTimeout: globalThis.clearTimeout,
    });
    invokeMock.mockImplementation((command: string) =>
      command === 'record_diagnostic_event'
        ? Promise.resolve()
        : Promise.reject(new Error('Falha api_key=segredo-chave')),
    );
    const report = vi.fn();

    await expect(startupInvoke('get_profile', undefined, report)).rejects.toThrow(
      'api_key=segredo-chave',
    );
    expect(report).toHaveBeenCalledWith(
      'get_profile',
      expect.any(String),
      expect.not.stringContaining('segredo-chave'),
    );
    expect(invokeMock).toHaveBeenCalledWith(
      'record_diagnostic_event',
      expect.objectContaining({
        section: 'startup',
        category: 'initialization',
        correlationId: expect.any(String),
      }),
    );
  });
});
