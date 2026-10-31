import { describe, expect, it } from 'vitest';
import { petWindowLayout } from './pet-layout';

describe('geometria da janela do pet', () => {
  it('mantém o modo Livre próximo do tamanho do mascote e acomoda o comando rápido expandido', () => {
    expect(petWindowLayout(true, false, 1)).toEqual({ width: 128, height: 128 });
    expect(petWindowLayout(true, true, 1)).toEqual({ width: 380, height: 330 });
  });

  it('dimensiona a cápsula da Ilha nos estados recolhido e expandido', () => {
    expect(petWindowLayout(false, false, 1)).toEqual({ width: 280, height: 104 });
    expect(petWindowLayout(false, true, 1.25)).toEqual({ width: 420, height: 360 });
  });
});
