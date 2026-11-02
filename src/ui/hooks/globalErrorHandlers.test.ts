// @vitest-environment jsdom

import type { ErrorInfo } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { upsertDiagnostic, type DiagnosticItem } from '../../core/diagnostics';
import { ScreenErrorBoundary } from '../ScreenErrorBoundary';
import { createGlobalErrorHandlers } from './globalErrorHandlers';

describe('encaminhamento de erros globais', () => {
  it('leva erro de renderização ao diagnóstico global e rejeição de ação só ao log', async () => {
    let bannerItems: DiagnosticItem[] = [];
    const reportSystemDiagnostic = vi.fn((command: string, message: string, details = '') => {
      bannerItems = upsertDiagnostic(bannerItems, command, message, details);
    });
    const record = vi.fn();
    const handlers = createGlobalErrorHandlers(
      reportSystemDiagnostic,
      record,
      () => '123e4567-e89b-12d3-a456-426614174000',
    );
    const onWindowError = handlers.onError as EventListener;
    window.addEventListener('error', onWindowError);

    const boundary = new ScreenErrorBoundary({ children: null });
    boundary.componentDidCatch(new Error('erro de renderização'), {
      componentStack: '\n    at CalendarScreen',
    } as ErrorInfo);
    window.removeEventListener('error', onWindowError);
    await Promise.resolve();

    expect(reportSystemDiagnostic).toHaveBeenCalledOnce();
    expect(reportSystemDiagnostic).toHaveBeenCalledWith(
      'frontend.window.onerror',
      'A interface encontrou um erro.',
      expect.stringContaining('erro de renderização'),
    );
    expect(bannerItems).toHaveLength(1);
    expect(bannerItems[0].command).toBe('frontend.window.onerror');

    handlers.onUnhandledRejection({
      reason: new Error('falha de comando da ação'),
    } as PromiseRejectionEvent);
    await Promise.resolve();

    expect(reportSystemDiagnostic).toHaveBeenCalledOnce();
    expect(bannerItems).toHaveLength(1);
    expect(record).toHaveBeenCalledWith(
      expect.objectContaining({
        event: 'frontend_unhandled_rejection',
        category: 'promise_rejection',
      }),
    );
  });
});
