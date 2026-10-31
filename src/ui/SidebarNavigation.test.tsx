import { readFileSync } from 'node:fs';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import {
  handleProfileMenuKey,
  navigationItems,
  nextProfileMenuFocus,
  PendingApprovalIndicator,
  positionProfileMenu,
  profileMenuItems,
  SidebarNavigation,
  SidebarProfileMenu,
  shouldShowStopAll,
} from './SidebarNavigation';

describe('SidebarNavigation', () => {
  it('não mantém o seletor do cabeçalho de grupo removido', () => {
    const styles = readFileSync(new URL('./styles.css', import.meta.url), 'utf8');
    expect(styles).not.toMatch(/\.nav-heading(?:[\s:{>+~])/);
  });

  it('mostra os cinco destinos principais na ordem acordada, sem cabeçalhos antigos', () => {
    expect(navigationItems.map(({ label }) => label)).toEqual([
      'Início',
      'Conversas',
      'Agenda',
      'Habilidades',
      'Conectores de IA',
    ]);
    const markup = renderToStaticMarkup(
      <SidebarNavigation
        section="overview"
        onSelectSection={() => undefined}
        onSetChatMode={() => undefined}
        keySavedForSession
      />,
    );
    expect(markup).toContain('aria-label="Navegação principal"');
    expect(markup).toContain('Chave disponível nesta sessão');
    expect(markup).not.toContain('ESPAÇO DE TRABALHO');
    expect(markup).not.toContain('PERSONALIZAÇÃO');
  });

  it('mantém apenas ícones com dicas acessíveis no modo compacto', () => {
    const markup = renderToStaticMarkup(
      <SidebarNavigation
        section="overview"
        onSelectSection={() => undefined}
        onSetChatMode={() => undefined}
        keySavedForSession={false}
        compact
      />,
    );
    expect(markup).toContain('title="Início"');
    expect(markup).toContain('aria-label="Conversas"');
    expect(markup).not.toContain('>Início</span>');
  });

  it('fixa o cartão do perfil no rodapé mesmo quando a lista de navegação rola', () => {
    const styles = readFileSync(new URL('./styles.css', import.meta.url), 'utf8');
    const markup = renderToStaticMarkup(
      <SidebarProfileMenu
        profile={{ displayName: '', nickname: '', avatarDataUrl: null }}
        compact={false}
        section="overview"
        onMyProfile={() => undefined}
        onSettings={() => undefined}
        onPermissions={() => undefined}
        onHelp={() => undefined}
        onAbout={() => undefined}
      />,
    );
    expect(markup).toContain('aria-haspopup="menu"');
    expect(markup).toContain('class="avatar-small">U');
    expect(styles).toMatch(/\.sidebar-profile-footer\s*\{[^}]*margin-top:\s*auto/s);
    expect(styles).toMatch(/\.nav-list\s*\{[^}]*overflow-y:\s*auto/s);
    expect(styles).toMatch(/\.sidebar-profile-popover\s*\{[^}]*position:\s*fixed/s);
  });

  it('posiciona o menu junto ao cartão expandido e limita a área visível', () => {
    expect(
      positionProfileMenu(
        { top: 700, right: 250, bottom: 748, left: 16 },
        { width: 270, height: 230 },
        { width: 1000, height: 800 },
        false,
      ),
    ).toEqual({ left: 16, top: 462, width: 270, maxHeight: 230 });
  });

  it('abre o menu compacto ao lado do cartão e o mantém dentro de uma janela estreita', () => {
    expect(
      positionProfileMenu(
        { top: 740, right: 72, bottom: 788, left: 8 },
        { width: 270, height: 230 },
        { width: 320, height: 600 },
        true,
      ),
    ).toEqual({ left: 80, top: 358, width: 228, maxHeight: 230 });
    expect(
      positionProfileMenu(
        { top: 740, right: 72, bottom: 788, left: 8 },
        { width: 270, height: 230 },
        { width: 240, height: 180 },
        true,
      ),
    ).toEqual({ left: 80, top: 12, width: 148, maxHeight: 156 });
  });

  it('declara as cinco ações do menu do perfil na ordem esperada', () => {
    expect(profileMenuItems.map(({ label }) => label)).toEqual([
      'Meu perfil',
      'Configurações',
      'Permissões e segurança',
      'Ajuda e suporte',
      'Sobre o OdeKrai',
    ]);
  });

  it('só mostra Parar tudo enquanto chat ou agente estão em execução', () => {
    expect(shouldShowStopAll(false, false)).toBe(false);
    expect(shouldShowStopAll(true, false)).toBe(true);
    expect(shouldShowStopAll(false, true)).toBe(true);
  });

  it('mostra o total de aprovações e não oferece controles de decisão', () => {
    const markup = renderToStaticMarkup(
      <PendingApprovalIndicator count={3} onOpen={() => undefined} />,
    );
    expect(markup).toContain('3 aprovações pendentes; abrir Atividade');
    expect(markup).toContain('>3</span>');
    expect(markup).not.toContain('Aprovar');
    expect(
      renderToStaticMarkup(<PendingApprovalIndicator count={0} onOpen={() => undefined} />),
    ).toBe('');
  });

  it('navega pelo menu com setas, Home e End e fecha com Escape devolvendo o foco', () => {
    const first = { focus: vi.fn(), click: vi.fn() };
    const last = { focus: vi.fn(), click: vi.fn() };
    const close = vi.fn();
    const restore = vi.fn();

    expect(nextProfileMenuFocus('ArrowDown', 4, 5)).toBe(0);
    expect(nextProfileMenuFocus('ArrowUp', 0, 5)).toBe(4);
    expect(nextProfileMenuFocus('Home', 3, 5)).toBe(0);
    expect(nextProfileMenuFocus('End', 1, 5)).toBe(4);
    expect(handleProfileMenuKey('ArrowDown', 0, [first, last], close, restore)).toBe(1);
    expect(last.focus).toHaveBeenCalledOnce();
    expect(handleProfileMenuKey('Enter', 1, [first, last], close, restore)).toBe(1);
    expect(last.click).toHaveBeenCalledOnce();
    expect(handleProfileMenuKey('Escape', 1, [first, last], close, restore)).toBe('close');
    expect(close).toHaveBeenCalledOnce();
    expect(restore).toHaveBeenCalledOnce();
  });
});
