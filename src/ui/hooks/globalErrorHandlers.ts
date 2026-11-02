import { sanitizeDiagnosticDetails } from '../../core/diagnostics';

type DiagnosticRecord = {
  event: 'frontend_window_error' | 'frontend_unhandled_rejection';
  command: 'window.onerror' | 'unhandledrejection';
  outcome: 'error';
  durationMs: 0;
  section: 'window';
  category: 'window_error' | 'promise_rejection';
  correlationId: string;
};

type ReportSystemDiagnostic = (command: string, message: string, details?: string) => void;
type RecordDiagnostic = (event: DiagnosticRecord) => Promise<unknown> | unknown;

function saveDiagnosticEvent(record: RecordDiagnostic, event: DiagnosticRecord) {
  try {
    void Promise.resolve(record(event)).catch(() =>
      console.warn('[OdeKrai diagnóstico] Não foi possível persistir um evento local.'),
    );
  } catch {
    console.warn('[OdeKrai diagnóstico] Não foi possível persistir um evento local.');
  }
}

export function createGlobalErrorHandlers(
  reportSystemDiagnostic: ReportSystemDiagnostic,
  record: RecordDiagnostic,
  createId: () => string = () => crypto.randomUUID(),
) {
  return {
    onError(event: ErrorEvent) {
      const id = createId();
      const details = sanitizeDiagnosticDetails(event.error ?? event.message);
      reportSystemDiagnostic(
        'frontend.window.onerror',
        'A interface encontrou um erro.',
        `Referência ${id}: ${details || 'O WebView não informou o motivo do erro.'}`,
      );
      saveDiagnosticEvent(record, {
        event: 'frontend_window_error',
        command: 'window.onerror',
        outcome: 'error',
        durationMs: 0,
        section: 'window',
        category: 'window_error',
        correlationId: id,
      });
    },
    onUnhandledRejection(_event: PromiseRejectionEvent) {
      // Rejeições podem vir de uma ação; a tela ou o comando responsável deve exibi-las localmente.
      const id = createId();
      saveDiagnosticEvent(record, {
        event: 'frontend_unhandled_rejection',
        command: 'unhandledrejection',
        outcome: 'error',
        durationMs: 0,
        section: 'window',
        category: 'promise_rejection',
        correlationId: id,
      });
    },
  };
}
