import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { defaultSettings } from '../../core/settings';
import { AssistantScreen, PetActionError } from './Assistant';

describe('AssistantScreen', () => {
  it('exibe falha ao criar o pet e oferece nova tentativa', () => {
    const markup = renderToStaticMarkup(
      <PetActionError
        message="Não foi possível criar a janela do pet."
        onRetry={() => undefined}
      />,
    );
    expect(markup).toContain('role="alert"');
    expect(markup).toContain('Não foi possível criar a janela do pet.');
    expect(markup).toContain('Tentar de novo');
  });

  it('monta as opções do mascote e os eventos sonoros agrupados', () => {
    const markup = renderToStaticMarkup(
      <AssistantScreen
        settings={defaultSettings}
        update={() => undefined}
        petVisible
        onPetVisibilityChange={() => undefined}
        onError={() => undefined}
        selectedPet="character-female-a"
        onSelectPet={() => undefined}
      />,
    );
    expect(markup).toContain('Escolha seu mascote');
    expect(markup).toContain('Nina');
    expect(markup).toContain(
      'aria-label="Nina, com pele morena, cabelo escuro preso e roupa lilás. Estado: Em espera."',
    );
    expect(markup).toContain('alt="Nina, com pele morena, cabelo escuro preso e roupa lilás"');
    expect(markup).toContain('Pet na');
    expect(markup).toContain('Modo do pet');
    expect(markup).toContain('>Ilha<');
    expect(markup).toContain('>Livre<');
    expect(markup).toContain('>Pet</h4>');
    expect(markup).toContain('>Chat</h4>');
    expect(markup).toContain('>Segurança</h4>');
    expect(markup).toContain('Aprovação necessária');
    expect(markup).toContain('Tocar prévia');
    expect(markup).toContain('Sem arquivo');
    expect(markup).toContain('Atalho para o comando rápido');
    expect(markup).toContain('aria-label="Atalho para o comando rápido"');
    expect(markup).not.toContain('checked=""');
  });

  it('não mostra configuração do atalho dentro do assistente de primeiro uso', () => {
    const markup = renderToStaticMarkup(
      <AssistantScreen
        settings={defaultSettings}
        update={() => undefined}
        petVisible={false}
        onPetVisibilityChange={() => undefined}
        onError={() => undefined}
        selectedPet="character-female-a"
        onSelectPet={() => undefined}
        embedded
      />,
    );
    expect(markup).not.toContain('Atalho para o comando rápido');
  });
});
