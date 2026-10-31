import type {
  ButtonHTMLAttributes,
  HTMLAttributes,
  InputHTMLAttributes,
  ReactNode,
  SelectHTMLAttributes,
} from 'react';
import { cloneElement, isValidElement, useId } from 'react';

export type ButtonVariant = 'primary' | 'secondary' | 'ghost' | 'danger';

export function Button({
  variant = 'secondary',
  className = '',
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & { variant?: ButtonVariant }) {
  return <button {...props} className={`ui-button ui-button--${variant} ${className}`.trim()} />;
}

export function Card({ className = '', ...props }: HTMLAttributes<HTMLElement>) {
  return <section {...props} className={`ui-card ${className}`.trim()} />;
}

export function Badge({
  tone = 'neutral',
  className = '',
  ...props
}: HTMLAttributes<HTMLSpanElement> & { tone?: 'neutral' | 'success' | 'warning' | 'danger' }) {
  return <span {...props} className={`ui-badge ui-badge--${tone} ${className}`.trim()} />;
}

export function Banner({
  tone = 'info',
  role = 'status',
  className = '',
  ...props
}: HTMLAttributes<HTMLDivElement> & { tone?: 'info' | 'warning' | 'danger' }) {
  return (
    <div {...props} role={role} className={`ui-banner ui-banner--${tone} ${className}`.trim()} />
  );
}

export type StatusState = 'ativa' | 'desativada' | 'pendente' | 'erro' | 'sucesso';

const statusLabels: Record<StatusState, string> = {
  ativa: 'Ativa',
  desativada: 'Desativada',
  pendente: 'Pendente',
  erro: 'Erro',
  sucesso: 'Sucesso',
};

export function StatusBadge({
  state,
  children,
  className = '',
  ...props
}: HTMLAttributes<HTMLSpanElement> & { state: StatusState }) {
  return (
    <span
      {...props}
      role="status"
      className={`ui-status-badge ui-status-badge--${state} ${className}`.trim()}
    >
      {children ?? statusLabels[state]}
    </span>
  );
}

export type TabItem = { id: string; label: string; content: ReactNode; disabled?: boolean };

export function getTabsKeyboardIndex(
  key: string,
  currentIndex: number,
  items: readonly Pick<TabItem, 'disabled'>[],
): number | null {
  if (!items.length || !['ArrowRight', 'ArrowLeft', 'Home', 'End'].includes(key)) return null;
  if (key === 'Home' || key === 'End') {
    const enabled = items
      .map((item, index) => (!item.disabled ? index : -1))
      .filter((index) => index >= 0);
    if (!enabled.length) return null;
    return key === 'Home' ? enabled[0] : enabled[enabled.length - 1];
  }
  const direction = key === 'ArrowRight' ? 1 : -1;
  for (let offset = 1; offset <= items.length; offset += 1) {
    const index = (currentIndex + direction * offset + items.length) % items.length;
    if (!items[index].disabled) return index;
  }
  return null;
}

export function Tabs({
  items,
  selectedId,
  onSelect,
  label,
  className = '',
}: {
  items: readonly TabItem[];
  selectedId: string;
  onSelect: (id: string) => void;
  label: string;
  className?: string;
}) {
  const idPrefix = useId();
  const selectedIndex = Math.max(
    0,
    items.findIndex((item) => item.id === selectedId),
  );
  const selected = items[selectedIndex];
  if (!selected) return null;
  return (
    <div className={`ui-tabs ${className}`.trim()}>
      <div className="ui-tabs-list" role="tablist" aria-label={label}>
        {items.map((item, index) => (
          <button
            key={item.id}
            id={`${idPrefix}-${item.id}-tab`}
            type="button"
            role="tab"
            aria-selected={item.id === selected.id}
            aria-controls={`${idPrefix}-${item.id}-panel`}
            tabIndex={item.id === selected.id ? 0 : -1}
            disabled={item.disabled}
            onClick={() => onSelect(item.id)}
            onKeyDown={(event) => {
              const nextIndex = getTabsKeyboardIndex(event.key, index, items);
              if (nextIndex === null) return;
              event.preventDefault();
              const next = items[nextIndex];
              onSelect(next.id);
              document.getElementById(`${idPrefix}-${next.id}-tab`)?.focus();
            }}
          >
            {item.label}
          </button>
        ))}
      </div>
      <div
        id={`${idPrefix}-${selected.id}-panel`}
        className="ui-tabs-panel"
        role="tabpanel"
        aria-labelledby={`${idPrefix}-${selected.id}-tab`}
        tabIndex={0}
      >
        {selected.content}
      </div>
    </div>
  );
}

export function Toast({
  children,
  action,
  onDismiss,
  className = '',
}: {
  children: ReactNode;
  action: { label?: string; onClick: () => void };
  onDismiss?: () => void;
  className?: string;
}) {
  return (
    <div className={`ui-toast ${className}`.trim()} role="status" aria-live="polite">
      <span>{children}</span>
      <button type="button" className="ui-button ui-button--ghost" onClick={action.onClick}>
        {action.label ?? 'Desfazer'}
      </button>
      {onDismiss && (
        <button
          type="button"
          className="ui-toast-dismiss"
          aria-label="Fechar aviso"
          onClick={onDismiss}
        >
          ×
        </button>
      )}
    </div>
  );
}

export function EmptyState({
  icon,
  mascot,
  title,
  description,
  action,
  className = '',
}: {
  icon?: ReactNode;
  mascot?: ReactNode;
  title: string;
  description: string;
  action?: { label: string; onClick: () => void };
  className?: string;
}) {
  const titleId = useId();
  return (
    <section className={`ui-empty-state ${className}`.trim()} aria-labelledby={titleId}>
      {(icon || mascot) && (
        <div className="ui-empty-state-visual" aria-hidden="true">
          {mascot ?? icon}
        </div>
      )}
      <h2 id={titleId}>{title}</h2>
      <p>{description}</p>
      {action && (
        <Button type="button" variant="primary" onClick={action.onClick}>
          {action.label}
        </Button>
      )}
    </section>
  );
}

export function Skeleton({
  width,
  height,
  className = '',
}: {
  width?: string;
  height?: string;
  className?: string;
}) {
  return (
    <span
      className={`ui-skeleton ${className}`.trim()}
      aria-hidden="true"
      style={{ width, height }}
    />
  );
}

export function Tooltip({
  content,
  children,
  className = '',
}: {
  content: string;
  children: ReactNode;
  className?: string;
}) {
  const id = useId();
  const trigger = isValidElement<{ 'aria-describedby'?: string }>(children) ? (
    cloneElement(children, {
      'aria-describedby': [children.props['aria-describedby'], id].filter(Boolean).join(' '),
    })
  ) : (
    <span className="ui-tooltip-trigger" aria-describedby={id} tabIndex={0}>
      {children}
    </span>
  );
  return (
    <span className={`ui-tooltip ${className}`.trim()}>
      <span className="ui-tooltip-trigger">{trigger}</span>
      <span className="ui-tooltip-content" id={id} role="tooltip">
        {content}
      </span>
    </span>
  );
}

export function Kbd({ children, className = '', ...props }: HTMLAttributes<HTMLElement>) {
  return (
    <kbd {...props} className={`ui-kbd ${className}`.trim()}>
      {children}
    </kbd>
  );
}

export function ListItem({
  leading,
  description,
  trailing,
  className = '',
  children,
  ...props
}: HTMLAttributes<HTMLLIElement> & {
  leading?: ReactNode;
  description?: ReactNode;
  trailing?: ReactNode;
}) {
  return (
    <li {...props} className={`ui-list-item ${className}`.trim()}>
      {leading && (
        <span className="ui-list-item-leading" aria-hidden="true">
          {leading}
        </span>
      )}
      <span className="ui-list-item-copy">
        <span className="ui-list-item-title">{children}</span>
        {description && <span className="ui-list-item-description">{description}</span>}
      </span>
      {trailing && <span className="ui-list-item-trailing">{trailing}</span>}
    </li>
  );
}

export function TextInput({ className = '', ...props }: InputHTMLAttributes<HTMLInputElement>) {
  return <input {...props} className={`ui-input ${className}`.trim()} />;
}

export function SelectInput({ className = '', ...props }: SelectHTMLAttributes<HTMLSelectElement>) {
  return <select {...props} className={`ui-select ${className}`.trim()} />;
}

export function Toggle({
  checked,
  label,
  className = '',
  children,
  ...props
}: Omit<ButtonHTMLAttributes<HTMLButtonElement>, 'onChange'> & {
  checked: boolean;
  label: string;
  children?: ReactNode;
}) {
  return (
    <button
      {...props}
      type="button"
      role="switch"
      aria-label={label}
      aria-checked={checked}
      className={`ui-toggle ${checked ? 'on' : ''} ${className}`.trim()}
    >
      {children ?? <span />}
    </button>
  );
}

export function SelectableCard({
  selected,
  className = '',
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & { selected: boolean }) {
  return (
    <button
      {...props}
      type="button"
      aria-pressed={selected}
      className={`ui-selectable-card ${selected ? 'selected' : ''} ${className}`.trim()}
    />
  );
}

export function PageHeader({
  eyebrow,
  title,
  subtitle,
  actions,
}: {
  eyebrow: string;
  title: string;
  subtitle?: string;
  actions?: ReactNode;
}) {
  return (
    <header className="page-heading">
      <div>
        <span className="eyebrow">{eyebrow}</span>
        <h1>{title}</h1>
        {subtitle && <p>{subtitle}</p>}
      </div>
      {actions && <div className="page-heading-actions">{actions}</div>}
    </header>
  );
}
