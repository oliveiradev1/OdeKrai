/** @vitest-environment jsdom */
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { afterEach } from 'vitest';
import type { CatalogSkill } from '../core/skills-catalog';
import { SkillsCatalogView } from './SkillsScreen';

const integrationSkills: CatalogSkill[] = [
  {
    id: 'google.calendar.list_events',
    name: 'Listar eventos do Google',
    description: 'Consulta eventos do calendário conectado.',
    risk: 'leitura',
    resultTrust: 'untrusted',
    enabled: false,
    morePermissive: false,
    fixedBlocked: false,
    category: 'integration',
    status: 'available',
    configurationRequired: false,
    family: 'google',
    icon: 'calendar',
    examples: ['Liste meus eventos de calendário.'],
    origin: { kind: 'builtin' },
  },
  {
    id: 'shardcloud.vm_list',
    name: 'Listar máquinas da nuvem',
    description: 'Consulta as máquinas autorizadas.',
    risk: 'leitura',
    resultTrust: 'untrusted',
    enabled: false,
    morePermissive: false,
    fixedBlocked: false,
    category: 'integration',
    status: 'available',
    configurationRequired: false,
    family: 'nuvem',
    icon: 'cloud',
    examples: ['Liste as VMs autorizadas.'],
    origin: { kind: 'builtin' },
  },
];

afterEach(cleanup);

describe('interações das famílias de habilidades', () => {
  it('usa um interruptor para todos os membros e preserva os interruptores individuais', () => {
    const onFamilyToggle = vi.fn();
    const { container, rerender } = render(
      <SkillsCatalogView
        skills={integrationSkills}
        onFamilyToggle={onFamilyToggle}
        onSkillToggle={vi.fn()}
        onOverrideToggle={vi.fn()}
      />,
    );

    expect(container.querySelectorAll('.skills-family-card')).toHaveLength(1);
    fireEvent.click(
      screen.getByRole('switch', { name: 'Ativar todas as habilidades de Integrações' }),
    );
    expect(onFamilyToggle).toHaveBeenLastCalledWith('integracoes', true);

    rerender(
      <SkillsCatalogView
        skills={integrationSkills.map((skill) => ({ ...skill, enabled: true }))}
        onFamilyToggle={onFamilyToggle}
        onSkillToggle={vi.fn()}
        onOverrideToggle={vi.fn()}
      />,
    );
    fireEvent.click(
      screen.getByRole('switch', { name: 'Desativar todas as habilidades de Integrações' }),
    );
    expect(onFamilyToggle).toHaveBeenLastCalledWith('integracoes', false);

    fireEvent.click(screen.getByRole('button', { name: 'Gerenciar Integrações' }));
    expect(container.querySelectorAll('.skills-detail-controls [role="switch"]')).toHaveLength(2);
    expect(
      Array.from(container.querySelectorAll('.skills-detail-controls [aria-label]')).map((item) =>
        item.getAttribute('aria-label'),
      ),
    ).toEqual(['Desativar Listar eventos do Google', 'Desativar Listar máquinas da nuvem']);
  });

  it('encontra uma habilidade pelo texto do exemplo', () => {
    const { container } = render(
      <SkillsCatalogView
        skills={integrationSkills}
        onFamilyToggle={vi.fn()}
        onSkillToggle={vi.fn()}
        onOverrideToggle={vi.fn()}
      />,
    );
    fireEvent.change(screen.getByLabelText('Procurar habilidades'), {
      target: { value: 'VMs autorizadas' },
    });
    expect(screen.getByText('1 habilidade encontrada')).toBeTruthy();
    expect(container.querySelectorAll('.skills-family-card')).toHaveLength(1);
    expect(screen.getByRole('heading', { name: 'Integrações' })).toBeTruthy();
  });
});
