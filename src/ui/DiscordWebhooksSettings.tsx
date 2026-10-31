import { useCallback, useEffect, useState } from 'react';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { Button } from './DesignSystem';
import { Icon } from './Icon';
import { submitSecretForm } from './secret-form';
import {
  revokeDiscordIntegration,
  saveDiscordIntegration,
  testDiscordConnection,
} from '../core/discord-integration';

type DiscordWebhook = { id: string; name: string };

export function DiscordWebhooksSettings({ onChanged }: { onChanged?: () => Promise<void> }) {
  const [webhooks, setWebhooks] = useState<DiscordWebhook[]>([]);
  const [name, setName] = useState('');
  const [url, setUrl] = useState('');
  const [authorized, setAuthorized] = useState(false);
  const [connectionTested, setConnectionTested] = useState(false);
  const [notice, setNotice] = useState('');
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);

  const refresh = useCallback(async () => {
    if (!isTauri()) return;
    try {
      setWebhooks(await invoke<DiscordWebhook[]>('discord_list'));
      setError('');
    } catch (cause) {
      setError(String(cause));
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  async function save(): Promise<boolean> {
    setBusy(true);
    try {
      await saveDiscordIntegration(name, url, authorized, connectionTested, invoke);
      setNotice('Integração configurada. A URL não será exibida novamente.');
      await refresh();
      try {
        await onChanged?.();
      } catch (cause) {
        setError(String(cause));
      }
      return true;
    } catch (cause) {
      setError(String(cause));
      return false;
    } finally {
      setBusy(false);
    }
  }

  async function testConnection() {
    setBusy(true);
    setConnectionTested(false);
    setNotice('');
    try {
      await testDiscordConnection(url, authorized, invoke);
      setConnectionTested(true);
      setNotice('Conexão confirmada. Nenhuma mensagem foi enviada.');
      setError('');
    } catch (cause) {
      setError(String(cause));
    } finally {
      setBusy(false);
    }
  }

  async function remove(webhook: DiscordWebhook) {
    setBusy(true);
    try {
      await revokeDiscordIntegration(webhook.id, invoke);
      setNotice('Destino removido do Discord e acesso local apagado.');
      await refresh();
      await onChanged?.();
    } catch (cause) {
      setError(String(cause));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section
      className="settings-panel settings-section allowed-folders-section"
      aria-labelledby="discord-integration-heading"
    >
      <h3 id="discord-integration-heading">Destino do Discord</h3>
      <p className="settings-description">
        Permite enviar mensagens ao canal conectado, sempre após aprovação da ação. Esta conexão não
        lê mensagens. Removê-la apaga o acesso salvo e pede confirmação; a ação não pode ser
        desfeita.
      </p>
      {error && (
        <div className="error-banner" role="alert">
          {error}
        </div>
      )}
      {notice && (
        <div role="status" className="success-banner">
          {notice}
        </div>
      )}
      <form
        onSubmit={(event) => {
          void submitSecretForm(event, save, () => {
            setName('');
            setUrl('');
            setAuthorized(false);
            setConnectionTested(false);
          });
        }}
      >
        <label className="discord-secret-field">
          Nome do destino
          <input
            id="discord-webhook-name"
            name="discord-webhook-name"
            value={name}
            maxLength={60}
            autoComplete="off"
            onChange={(event) => {
              setName(event.target.value);
              setConnectionTested(false);
            }}
          />
        </label>
        <label className="discord-secret-field">
          Endereço de conexão do Discord
          <input
            id="discord-webhook-url"
            name="discord-webhook-url"
            type="password"
            autoComplete="off"
            value={url}
            onChange={(event) => {
              setUrl(event.target.value);
              setConnectionTested(false);
            }}
          />
        </label>
        <label className="discord-consent">
          <input
            type="checkbox"
            checked={authorized}
            onChange={(event) => setAuthorized(event.target.checked)}
          />
          Autorizo enviar os dados de conexão ao Discord para verificar este destino e salvar o
          acesso no Gerenciador de Credenciais do Windows. Cada mensagem ainda pede aprovação.
        </label>
        <div className="folder-shortcuts">
          <Button
            variant="secondary"
            disabled={busy || !isTauri() || !url || !authorized}
            type="button"
            onClick={() => void testConnection()}
          >
            Testar conexão
          </Button>
          <Button
            variant="secondary"
            type="submit"
            disabled={
              busy || !isTauri() || !name.trim() || !url || !authorized || !connectionTested
            }
          >
            Autorizar e salvar
          </Button>
        </div>
      </form>
      <ul className="allowed-folder-list">
        {webhooks.map((webhook) => (
          <li key={webhook.id}>
            <div>
              <strong>{webhook.name}</strong>
            </div>
            <Button
              variant="ghost"
              type="button"
              disabled={busy}
              onClick={() => void remove(webhook)}
              aria-label={`Remover destino ${webhook.name} do Discord e do aplicativo`}
            >
              <Icon name="delete" />
            </Button>
          </li>
        ))}
      </ul>
    </section>
  );
}
