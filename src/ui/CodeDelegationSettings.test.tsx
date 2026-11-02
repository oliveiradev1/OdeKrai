import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { CodeDelegationSettings } from './CodeDelegationSettings';

describe('delegação de código', () => {
  it('explica que a execução está desativada enquanto não houver isolamento do sistema', () => {
    const markup = renderToStaticMarkup(<CodeDelegationSettings />);

    expect(markup).toContain('temporariamente desativada');
    expect(markup).toContain('worktree não é');
    expect(markup).toContain('sandbox');
    expect(markup).not.toContain('Selecionar executável');
    expect(markup).not.toContain('<select');
  });
});
