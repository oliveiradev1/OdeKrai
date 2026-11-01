import { Icon } from './Icon';

type LocalServiceStatus = {
  label: string;
  tone: 'connected' | 'warning' | 'disconnected';
};

type Props = {
  status: LocalServiceStatus;
  port: string;
  busy: boolean;
  onPortChange: (port: string) => void;
  onProbe: () => void;
  onOpenComputer: () => void;
};

export function LocalAiProviderCard({
  status,
  port,
  busy,
  onPortChange,
  onProbe,
  onOpenComputer,
}: Props) {
  return (
    <article className="provider-card provider-local-free-card">
      <div className="provider-tile">
        <Icon name="harddrives" size={23} />
        <span className="provider-tile-copy">
          <strong>IA local (gratuita)</strong>
          <small>Execute modelos no próprio computador.</small>
        </span>
        <span className="provider-badges">
          <span className={`provider-badge--${status.tone}`} role="status">
            {status.label}
          </span>
        </span>
      </div>
      <label className="field-label">
        Porta local adicional (opcional)
        <input
          type="number"
          min={1024}
          max={65535}
          step={1}
          value={port}
          onChange={(event) => onPortChange(event.target.value)}
          placeholder="Ex.: 9931"
          aria-describedby="local-ai-port-help"
        />
      </label>
      <small id="local-ai-port-help">
        A busca usa somente 127.0.0.1 e também verifica as portas padrão.
      </small>
      <button type="button" className="provider-local-action" disabled={busy} onClick={onProbe}>
        Procurar IA local
      </button>
      <button type="button" className="provider-local-action" onClick={onOpenComputer}>
        Abrir Meu computador
      </button>
    </article>
  );
}
