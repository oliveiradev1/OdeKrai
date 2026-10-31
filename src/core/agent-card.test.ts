import { describe, expect, it } from 'vitest';
import {
  formatRemainingApprovalTime,
  getAgentCardState,
  isSafeArtifactPath,
  setActionCardUndoError,
} from './agent-card';

describe('contrato visual do cartão do agente', () => {
  it.each([
    ['Planejada', 'planejada'],
    ['Aguardando aprovação', 'aguardando aprovação'],
    ['Configuração pendente', 'configuração pendente'],
    ['Executando', 'executando'],
    ['Concluída', 'concluída'],
    ['Negada', 'negada'],
    ['Expirada', 'expirada'],
    ['Falhou', 'erro'],
  ])('normaliza %s para %s', (status, expected) => {
    expect(getAgentCardState({ status })).toBe(expected);
  });

  it('recusa travessia de diretório e executáveis nos artefatos', () => {
    expect(isSafeArtifactPath('C:\\workspace\\resultado.txt')).toBe(true);
    expect(isSafeArtifactPath('../segredo.txt')).toBe(false);
    expect(isSafeArtifactPath('C:\\workspace\\abrir.exe')).toBe(false);
    expect(isSafeArtifactPath('C:\\workspace\\script.ps1')).toBe(false);
    expect(isSafeArtifactPath('C:\\workspace\\null\u0000.txt')).toBe(false);
  });

  it('formata prazo restante e prazo vencido', () => {
    expect(formatRemainingApprovalTime(61, 0)).toBe('1:01 restantes');
    expect(formatRemainingApprovalTime(1, 2000)).toBe('Prazo expirado');
    expect(formatRemainingApprovalTime(undefined)).toBeNull();
  });

  it('limita uma falha ao cartão que originou a restauração', () => {
    const cards = [{ id: 'origem' }, { id: 'outra-ação' }];
    const result = setActionCardUndoError(cards, 'origem', 'Parâmetros inválidos.');

    expect(result[0]).toEqual({ id: 'origem', undoError: 'Parâmetros inválidos.' });
    expect(result[1]).toBe(cards[1]);
  });
});
