import { useEffect, useRef, useState, type KeyboardEvent } from 'react';
import { createPortal } from 'react-dom';
import { initials, type ProfileData } from './Profile';
import { Icon, type IconName } from './Icon';
import type { Section } from './screens/types';
import { normalizeProfileData } from '../core/identity';

export const navigationItems: { id: Section; label: string; icon: IconName }[] = [
  { id: 'overview', label: 'Início', icon: 'home' },
  { id: 'chat', label: 'Conversas', icon: 'chat' },
  { id: 'calendar', label: 'Agenda', icon: 'calendar' },
  { id: 'skills', label: 'Habilidades', icon: 'wrench' },
  { id: 'providers', label: 'Conectores de IA', icon: 'connectors' },
];

export const profileMenuItems = [
  { id: 'profile', label: 'Meu perfil', icon: 'profile' },
  { id: 'settings', label: 'Configurações', icon: 'settings' },
  { id: 'permissions', label: 'Permissões e segurança', icon: 'shield' },
  { id: 'help', label: 'Ajuda e suporte', icon: 'help' },
  { id: 'about', label: 'Sobre o OdeKrai', icon: 'info' },
] as const satisfies readonly { id: string; label: string; icon: IconName }[];

export function nextProfileMenuFocus(
  key: string,
  currentIndex: number,
  itemCount: number,
): number | 'close' | null {
  if (!itemCount) return key === 'Escape' ? 'close' : null;
  if (key === 'Escape') return 'close';
  if (key === 'Home') return 0;
  if (key === 'End') return itemCount - 1;
  if (key === 'ArrowDown') return (currentIndex + 1 + itemCount) % itemCount;
  if (key === 'ArrowUp') return (currentIndex - 1 + itemCount) % itemCount;
  return null;
}

export type ProfileMenuAnchorRect = Pick<DOMRect, 'top' | 'right' | 'bottom' | 'left'>;

export function positionProfileMenu(
  anchor: ProfileMenuAnchorRect,
  menu: { width: number; height: number },
  viewport: { width: number; height: number },
  compact: boolean,
) {
  const margin = 12;
  const gap = 8;
  let width = Math.min(menu.width, Math.max(0, viewport.width - margin * 2));
  const height = Math.min(menu.height, Math.max(0, viewport.height - margin * 2));
  const rightSpace = viewport.width - margin - (anchor.right + gap);
  const leftSpace = anchor.left - gap - margin;
  let left = anchor.left;
  if (compact && rightSpace > 0) {
    width = Math.min(width, rightSpace);
    left = anchor.right + gap;
  } else if (compact && leftSpace > 0) {
    width = Math.min(width, leftSpace);
    left = anchor.left - gap - width;
  }
  const topCandidate = compact ? anchor.bottom - height : anchor.top - gap - height;
  const top = compact || topCandidate >= margin ? topCandidate : anchor.bottom + gap;

  return {
    left: Math.min(Math.max(margin, left), Math.max(margin, viewport.width - width - margin)),
    top: Math.min(Math.max(margin, top), Math.max(margin, viewport.height - height - margin)),
    width,
    maxHeight: height,
  };
}

export function shouldShowStopAll(busy: boolean, agentRunning: boolean): boolean {
  return busy || agentRunning;
}

export function handleProfileMenuKey(
  key: string,
  currentIndex: number,
  items: Array<Pick<HTMLButtonElement, 'focus' | 'click'> | null>,
  close: () => void,
  restoreTriggerFocus: () => void,
): number | 'close' | null {
  if (key === 'Enter' && currentIndex >= 0) {
    items[currentIndex]?.click();
    return currentIndex;
  }
  const destination = nextProfileMenuFocus(key, currentIndex, items.length);
  if (destination === 'close') {
    close();
    restoreTriggerFocus();
    return destination;
  }
  if (destination !== null) items[destination]?.focus();
  return destination;
}

export function SidebarNavigation({
  section,
  onSelectSection,
  onSetChatMode,
  keySavedForSession,
  compact = false,
}: {
  section: Section;
  onSelectSection: (section: Section) => void;
  onSetChatMode: (enabled: boolean) => void;
  keySavedForSession: boolean;
  compact?: boolean;
}) {
  return (
    <nav
      className={`nav-list ${compact ? 'nav-list-compact' : ''}`}
      aria-label="Navegação principal"
    >
      {navigationItems.map((item) => (
        <button
          className={`nav-item ${section === item.id ? 'active' : ''}`}
          key={item.id}
          type="button"
          title={compact ? item.label : undefined}
          aria-label={compact ? item.label : undefined}
          aria-current={section === item.id ? 'page' : undefined}
          onClick={() => {
            onSelectSection(item.id);
            onSetChatMode(false);
          }}
        >
          <span className="nav-icon">
            <Icon name={item.icon} size={20} />
          </span>
          {!compact && <span>{item.label}</span>}
          {item.id === 'providers' && (
            <span
              className={`nav-status ${keySavedForSession ? 'configured' : ''}`}
              role="img"
              title={keySavedForSession ? 'Chave disponível nesta sessão' : 'Chave não configurada'}
              aria-label={
                keySavedForSession ? 'Chave disponível nesta sessão' : 'Chave não configurada'
              }
            />
          )}
        </button>
      ))}
    </nav>
  );
}

export function SidebarProfileMenu({
  profile,
  compact,
  section,
  onMyProfile,
  onSettings,
  onPermissions,
  onHelp,
  onAbout,
}: {
  profile: ProfileData;
  compact: boolean;
  section: Section;
  onMyProfile: () => void;
  onSettings: () => void;
  onPermissions: () => void;
  onHelp: () => void;
  onAbout: () => void;
}) {
  const [open, setOpen] = useState(false);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const itemRefs = useRef<Array<HTMLButtonElement | null>>([]);
  const positionedMenuRef = useRef(false);
  const [menuPosition, setMenuPosition] = useState<ReturnType<typeof positionProfileMenu> | null>(
    null,
  );
  const safeProfile = normalizeProfileData(profile);
  const displayName = safeProfile.displayName.trim() || 'Seu perfil';

  useEffect(() => {
    if (!open) {
      positionedMenuRef.current = false;
      return;
    }
    if (menuPosition && !positionedMenuRef.current) {
      positionedMenuRef.current = true;
      itemRefs.current[0]?.focus();
    }
  }, [open, menuPosition]);

  useEffect(() => {
    if (!open) {
      setMenuPosition(null);
      return;
    }
    const trigger = triggerRef.current;
    const menu = menuRef.current;
    if (!trigger || !menu) return;
    let frame = 0;
    const updatePosition = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => {
        const anchor = trigger.getBoundingClientRect();
        const bounds = menu.getBoundingClientRect();
        setMenuPosition(
          positionProfileMenu(
            anchor,
            { width: Math.max(bounds.width, 270), height: bounds.height },
            { width: window.innerWidth, height: window.innerHeight },
            compact,
          ),
        );
      });
    };
    const resizeObserver =
      typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(updatePosition);
    resizeObserver?.observe(trigger);
    resizeObserver?.observe(menu);
    window.addEventListener('resize', updatePosition);
    window.addEventListener('scroll', updatePosition, true);
    window.visualViewport?.addEventListener('resize', updatePosition);
    window.visualViewport?.addEventListener('scroll', updatePosition);
    updatePosition();
    return () => {
      cancelAnimationFrame(frame);
      resizeObserver?.disconnect();
      window.removeEventListener('resize', updatePosition);
      window.removeEventListener('scroll', updatePosition, true);
      window.visualViewport?.removeEventListener('resize', updatePosition);
      window.visualViewport?.removeEventListener('scroll', updatePosition);
    };
  }, [open, compact]);

  useEffect(() => {
    setOpen(false);
  }, [section]);

  useEffect(() => {
    if (!open) return;
    const closeOnOutsidePointer = (event: PointerEvent) => {
      if (
        event.target instanceof Node &&
        !triggerRef.current?.contains(event.target) &&
        !menuRef.current?.contains(event.target)
      )
        setOpen(false);
    };
    document.addEventListener('pointerdown', closeOnOutsidePointer);
    return () => document.removeEventListener('pointerdown', closeOnOutsidePointer);
  }, [open]);

  function handleMenuKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    const currentIndex = itemRefs.current.findIndex((item) => item === document.activeElement);
    const destination = handleProfileMenuKey(
      event.key,
      Math.max(0, currentIndex),
      itemRefs.current,
      () => setOpen(false),
      () => window.requestAnimationFrame(() => triggerRef.current?.focus()),
    );
    if (destination !== null) event.preventDefault();
  }

  const actions = {
    profile: onMyProfile,
    settings: onSettings,
    permissions: onPermissions,
    help: onHelp,
    about: onAbout,
  };

  return (
    <div className="sidebar-profile-footer">
      <button
        ref={triggerRef}
        className="profile-card sidebar-profile-trigger"
        type="button"
        title={compact ? `Perfil: ${displayName}` : undefined}
        aria-label={`Abrir menu do perfil: ${displayName}`}
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={open ? 'sidebar-profile-menu' : undefined}
        onClick={() => setOpen((current) => !current)}
        onKeyDown={(event) => {
          if (event.key === 'ArrowDown' && !open) {
            event.preventDefault();
            setOpen(true);
          }
        }}
      >
        <span className="avatar-small">
          {safeProfile.avatarDataUrl ? (
            <img src={safeProfile.avatarDataUrl} alt="" />
          ) : (
            initials(safeProfile.displayName)
          )}
          <span className="online-dot" />
        </span>
        <span className="profile-copy">
          <strong>{displayName}</strong>
          <span>{safeProfile.nickname.trim() || 'Editar perfil'}</span>
        </span>
      </button>
      {open &&
        createPortal(
          <div
            ref={menuRef}
            id="sidebar-profile-menu"
            className="sidebar-profile-popover"
            role="menu"
            aria-label="Menu do perfil"
            onKeyDown={handleMenuKeyDown}
            style={{
              left: menuPosition?.left ?? 0,
              top: menuPosition?.top ?? 0,
              width: menuPosition?.width,
              maxHeight: menuPosition?.maxHeight,
              visibility: menuPosition ? 'visible' : 'hidden',
            }}
          >
            {profileMenuItems.map((item, index) => (
              <button
                ref={(element) => {
                  itemRefs.current[index] = element;
                }}
                key={item.id}
                type="button"
                role="menuitem"
                onClick={() => {
                  setOpen(false);
                  triggerRef.current?.focus();
                  actions[item.id]();
                }}
              >
                <Icon name={item.icon} size={18} />
                <span>{item.label}</span>
              </button>
            ))}
          </div>,
          document.body,
        )}
    </div>
  );
}

export function PendingApprovalIndicator({ count, onOpen }: { count: number; onOpen: () => void }) {
  if (count < 1) return null;
  const label = `${count} ${count === 1 ? 'aprovação pendente' : 'aprovações pendentes'}; abrir Atividade`;
  return (
    <button
      type="button"
      className="pending-approval-indicator"
      aria-label={label}
      title={label}
      onClick={onOpen}
    >
      <span aria-hidden="true">{count}</span>
      <span>Aprovações</span>
    </button>
  );
}
