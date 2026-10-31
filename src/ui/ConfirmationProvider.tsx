import {
  Component,
  createContext,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from 'react';
import type { ConfirmationRequest } from '../core/confirmation';
import { Button } from './DesignSystem';
import { Modal } from './Modal';

type PendingConfirmation = ConfirmationRequest & {
  id: number;
  resolve: (accepted: boolean) => void;
};
type ConfirmationContextValue = (request: ConfirmationRequest) => Promise<boolean>;

class ConfirmationRenderBoundary extends Component<
  {
    children: ReactNode;
    onError: () => void;
  },
  { failed: boolean }
> {
  state = { failed: false };

  static getDerivedStateFromError() {
    return { failed: true };
  }

  componentDidCatch() {
    this.props.onError();
  }

  render() {
    return this.state.failed ? null : this.props.children;
  }
}

const ConfirmationContext = createContext<ConfirmationContextValue | null>(null);

export function ConfirmationDialog({
  request,
  onCancel,
  onConfirm,
}: {
  request: ConfirmationRequest;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  return (
    <Modal open title={request.title} onClose={onCancel}>
      <ConfirmationDialogContent request={request} onCancel={onCancel} onConfirm={onConfirm} />
    </Modal>
  );
}

export function ConfirmationDialogContent({
  request,
  onCancel,
  onConfirm,
}: {
  request: ConfirmationRequest;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  return (
    <div className="confirmation-dialog-content">
      <p className="confirmation-dialog-message">{request.message}</p>
      <footer className="confirmation-dialog-actions">
        <Button
          type="button"
          variant="secondary"
          data-autofocus={request.destructive ? '' : undefined}
          onClick={onCancel}
        >
          Cancelar
        </Button>
        <Button
          type="button"
          variant={request.destructive ? 'danger' : 'primary'}
          onClick={onConfirm}
        >
          Confirmar
        </Button>
      </footer>
    </div>
  );
}

export function useConfirmation(): ConfirmationContextValue {
  const confirm = useContext(ConfirmationContext);
  return (
    confirm ??
    (() => Promise.reject(new Error('A confirmação interna não está disponível nesta tela.')))
  );
}

export function ConfirmationProvider({ children }: { children: ReactNode }) {
  const [pending, setPending] = useState<PendingConfirmation | null>(null);
  const [failure, setFailure] = useState('');
  const pendingRef = useRef<PendingConfirmation | null>(null);
  const requestIdRef = useRef(0);

  const confirm = useCallback((request: ConfirmationRequest) => {
    if (typeof document === 'undefined' || !document.body) {
      setFailure('Não foi possível mostrar a confirmação. A ação não foi realizada.');
      return Promise.resolve(false);
    }
    if (pendingRef.current) {
      setFailure('Já existe uma confirmação aberta. A nova ação não foi realizada.');
      return Promise.resolve(false);
    }
    setFailure('');
    return new Promise<boolean>((resolve) => {
      const next = { ...request, id: ++requestIdRef.current, resolve };
      pendingRef.current = next;
      setPending(next);
    });
  }, []);

  const finish = useCallback((accepted: boolean) => {
    const current = pendingRef.current;
    if (!current) return;
    pendingRef.current = null;
    setPending(null);
    current.resolve(accepted);
  }, []);

  useEffect(
    () => () => {
      const current = pendingRef.current;
      pendingRef.current = null;
      current?.resolve(false);
    },
    [],
  );

  return (
    <ConfirmationContext.Provider value={confirm}>
      {children}
      {failure && (
        <div className="toast" role="alert">
          {failure}
        </div>
      )}
      {pending && (
        <ConfirmationRenderBoundary
          key={pending.id}
          onError={() => {
            setFailure('Não foi possível mostrar a confirmação. A ação não foi realizada.');
            finish(false);
          }}
        >
          <ConfirmationDialog
            request={pending}
            onCancel={() => finish(false)}
            onConfirm={() => finish(true)}
          />
        </ConfirmationRenderBoundary>
      )}
    </ConfirmationContext.Provider>
  );
}
