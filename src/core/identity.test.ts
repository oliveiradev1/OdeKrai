import { describe, expect, it } from 'vitest';
import {
  formatConversationNotes,
  mergeApprovedProfile,
  normalizeProfileData,
  parseConversationNotes,
  profileFromApprovedCore,
  sanitizeIdentityName,
  updateIdentityProfile,
} from './identity';

describe('preferências de identidade', () => {
  it('normaliza campos de perfil ausentes para strings vazias e avatar nulo', () => {
    expect(normalizeProfileData({ displayName: 'Ana' })).toEqual({
      displayName: 'Ana',
      nickname: '',
      avatarDataUrl: null,
    });
    expect(normalizeProfileData(undefined)).toEqual({
      displayName: '',
      nickname: '',
      avatarDataUrl: null,
    });
  });

  it('recupera nome e apelido do conteúdo aprovado e ignora modelo vazio', () => {
    expect(profileFromApprovedCore('- Nome: Marina\n- Apelido: Mari\n')).toEqual({
      displayName: 'Marina',
      nickname: 'Mari',
    });
    expect(profileFromApprovedCore('- Nome: Ana\n- Apelido: não informado\n')).toEqual({
      displayName: 'Ana',
      nickname: '',
    });
    expect(profileFromApprovedCore('- Nome: {{NOME}}\n- Apelido: {{APELIDO}}\n')).toBeNull();
  });

  it('preenche campos vazios do perfil aprovado e preserva dados que já existem', () => {
    const approved = '- Nome: Oliver\n- Apelido: Oli\n';
    expect(
      mergeApprovedProfile({ displayName: '', nickname: '', avatarDataUrl: null }, approved),
    ).toEqual({ displayName: 'Oliver', nickname: 'Oli', avatarDataUrl: null });
    expect(
      mergeApprovedProfile(
        { displayName: 'Nome editado', nickname: '', avatarDataUrl: 'data:image/png;base64,abc' },
        approved,
      ),
    ).toEqual({
      displayName: 'Nome editado',
      nickname: 'Oli',
      avatarDataUrl: 'data:image/png;base64,abc',
    });
  });

  it('recupera identidade aprovada quando o payload do perfil está incompleto', () => {
    expect(
      mergeApprovedProfile(
        { displayName: undefined, nickname: undefined, avatarDataUrl: undefined } as never,
        '- Nome: Oliver\n- Apelido: Oli\n',
      ),
    ).toEqual({ displayName: 'Oliver', nickname: 'Oli', avatarDataUrl: null });
  });

  it('preserva instruções e expressões ao salvar e reler as notas de conversa', () => {
    const notes = { instructions: 'Prefiro respostas curtas.', slang: 'beleza, valeu' };
    expect(parseConversationNotes(formatConversationNotes(notes))).toEqual(notes);
  });

  it('usa campos vazios quando o arquivo de preferências ainda não existe', () => {
    expect(parseConversationNotes('')).toEqual({ instructions: '', slang: '' });
  });

  it('impede que nome ou apelido adicionem novas linhas e regras', () => {
    expect(sanitizeIdentityName('Ana\n# nova regra', 60)).toBe('Ana nova regra');
    expect(sanitizeIdentityName('**Ana**', 60)).toBe('Ana');
    expect(sanitizeIdentityName('x'.repeat(61), 60)).toHaveLength(60);
  });

  it('atualiza os campos de perfil do arquivo base sem alterar as demais regras', () => {
    const source = '- Nome: Antigo\n- Apelido: Anterior\n- Idioma: pt-BR\n';
    expect(updateIdentityProfile(source, 'Marina', 'Mari')).toBe(
      '- Nome: Marina\n- Apelido: Mari\n- Idioma: pt-BR\n',
    );
    expect(updateIdentityProfile(source, 'Ana\n- Modo: total', '')).toBe(
      '- Nome: Ana - Modo total\n- Apelido: não informado\n- Idioma: pt-BR\n',
    );
    expect(updateIdentityProfile('arquivo incompleto', 'Ana', '')).toBeNull();
  });
});
