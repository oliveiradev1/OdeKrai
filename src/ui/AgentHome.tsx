import { useEffect, useState, type FormEvent } from 'react';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { Banner, Button, Card, PageHeader, SelectInput, TextInput } from './DesignSystem';
import { Profile, type ProfileData } from './Profile';
import {
  formatConversationNotes,
  parseConversationNotes,
  updateIdentityProfile,
} from '../core/identity';
import { memoryDeletionConfirmation, runConfirmedAction } from '../core/confirmation';
import { useConfirmation } from './ConfirmationProvider';

const TONE_PRESETS = ['direto e cordial', 'acolhedor', 'formal'];

type HomeStatus = {
  configured: boolean;
  path: string;
  state: string;
  message: string | null;
  warning: string | null;
  changed_externally: boolean;
};
type CoreRead = {
  content: string;
  current_hash: string;
  approved_content: string;
  changed_externally: boolean;
  diff: string;
};
type IdentityDraft = {
  name: string;
  nickname: string;
  language: string;
  tone: string;
  mode: string;
};

function withTimeout<T>(promise: Promise<T>, message: string): Promise<T> {
  let timer: number | undefined;
  return Promise.race([
    promise,
    new Promise<T>((_, reject) => {
      timer = window.setTimeout(() => reject(new Error(message)), 10000);
    }),
  ]).finally(() => {
    if (timer !== undefined) window.clearTimeout(timer);
  });
}

export function AgentHome({
  onStatus,
  initialName = '',
  initialNickname = '',
  profileValue,
  onProfileChange,
  embedded = false,
}: {
  onStatus?: (status: HomeStatus) => void;
  initialName?: string;
  initialNickname?: string;
  profileValue: ProfileData;
  onProfileChange: (profile: ProfileData) => void;
  embedded?: boolean;
}) {
  const requestConfirmation = useConfirmation();
  const [status, setStatus] = useState<HomeStatus | null>(null);
  const [path, setPath] = useState('');
  const [customTone, setCustomTone] = useState(false);
  const [profile, setProfile] = useState<IdentityDraft>({
    name: initialName,
    nickname: initialNickname,
    language: 'pt-BR',
    tone: 'direto e cordial',
    mode: 'sempre_perguntar',
  });
  const [core, setCore] = useState<CoreRead | null>(null);
  const [coreDraft, setCoreDraft] = useState('');
  const [names, setNames] = useState<string[]>([]);
  const [memoryName, setMemoryName] = useState('');
  const [memoryDraft, setMemoryDraft] = useState('');
  const [error, setError] = useState('');
  const [notice, setNotice] = useState('');
  const [busy, setBusy] = useState(false);
  const [setupStep, setSetupStep] = useState(1);
  const [personalInstructions, setPersonalInstructions] = useState('');
  const [slang, setSlang] = useState('');

  async function refresh() {
    const next = await Promise.race([
      invoke<HomeStatus>('agent_home_status'),
      new Promise<never>((_, reject) =>
        window.setTimeout(
          () => reject(new Error('A verificação da pasta excedeu o tempo limite.')),
          8000,
        ),
      ),
    ]);
    setStatus(next);
    setPath(next.path);
    onStatus?.(next);
    if (next.configured) {
      const [document, files] = await Promise.all([
        invoke<CoreRead>('agent_home_read_core'),
        invoke<string[]>('agent_memory_list'),
      ]);
      setCore(document);
      setCoreDraft(document.content);
      setNames(files.filter((name) => name !== 'estilo.md'));
      const style = await invoke<string>('agent_memory_read', { name: 'estilo.md' }).catch(
        () => '',
      );
      const notes = parseConversationNotes(style);
      setPersonalInstructions(notes.instructions);
      setSlang(notes.slang);
      const language = document.content.match(/^- Idioma: (.*)$/m)?.[1];
      const tone = document.content.match(/^- Tom: (.*)$/m)?.[1];
      if (language) setProfile((current) => ({ ...current, language }));
      if (tone) {
        setProfile((current) => ({ ...current, tone }));
        setCustomTone(!TONE_PRESETS.includes(tone));
      }
    }
  }
  useEffect(() => {
    if (isTauri()) {
      void withTimeout(refresh(), 'O carregamento das preferências excedeu o tempo limite.').catch(
        (cause) => setError(String(cause)),
      );
    }
  }, []);
  useEffect(() => {
    setProfile((current) => ({
      ...current,
      name: current.name || initialName,
      nickname: current.nickname || initialNickname,
    }));
  }, [initialName, initialNickname]);
  async function run(action: () => Promise<void>) {
    setBusy(true);
    setError('');
    setNotice('');
    try {
      await action();
    } catch (cause) {
      setError(String(cause));
    } finally {
      setBusy(false);
    }
  }
  async function confirmAction(request: Parameters<typeof requestConfirmation>[0]) {
    try {
      return await requestConfirmation(request);
    } catch {
      setError('Não foi possível mostrar a confirmação. A ação não foi realizada.');
      return false;
    }
  }
  async function initialize(event: FormEvent) {
    event.preventDefault();
    if (setupStep < 3) {
      setSetupStep((step) => step + 1);
      return;
    }
    if (
      !(await confirmAction({
        title: 'Salvar identidade?',
        message: `Salvar sua identidade e criar a pasta do OdeKrai em ${path}?`,
      }))
    )
      return;
    await run(async () => {
      await invoke('agent_home_init', { path, profile });
      const saved = await invoke<ProfileData>('save_profile', {
        displayName: profile.name,
        nickname: profile.nickname,
      });
      onProfileChange(saved);
      await refresh();
      setNotice('Identidade salva. Você pode ajustar as permissões na tela Permissões.');
    });
  }
  async function saveCore() {
    if (!core) return;
    if (
      !(await confirmAction({
        title: 'Confirmar arquivo base?',
        message: 'Confirmar este arquivo base como sua identidade?',
      }))
    )
      return;
    await run(async () => {
      await invoke('agent_home_save_core', { content: coreDraft, expectedHash: core.current_hash });
      await refresh();
      setNotice('Arquivo base confirmado e salvo.');
    });
  }
  async function saveConversationPreferences() {
    await run(async () => {
      const current = await invoke<CoreRead>('agent_home_read_core');
      if (current.changed_externally) {
        throw new Error('Confira e confirme primeiro as alterações do arquivo base.');
      }
      const next = current.content
        .replace(/^- Idioma: .*$/m, `- Idioma: ${profile.language}`)
        .replace(/^- Tom: .*$/m, `- Tom: ${profile.tone}`);
      if (next !== current.content) {
        if (
          !(await confirmAction({
            title: 'Salvar preferências?',
            message: 'Salvar o idioma e o tom no arquivo base?',
          }))
        )
          return;
        await invoke('agent_home_save_core', { content: next, expectedHash: current.current_hash });
      }
      const content = formatConversationNotes({ instructions: personalInstructions, slang });
      await invoke('agent_memory_save', { name: 'estilo.md', content });
      await refresh();
      setNotice('Preferências de conversa salvas.');
    });
  }
  async function approveProfileChange(next: ProfileData) {
    const current = await invoke<CoreRead>('agent_home_read_core');
    const updated = updateIdentityProfile(current.content, next.displayName, next.nickname);
    if (!updated) throw new Error('Confira o nome e o apelido no perfil.');
    if (updated !== current.content || current.changed_externally) {
      const externalReview = current.changed_externally
        ? `\n\nO arquivo também mudou fora do aplicativo:\n${current.diff}`
        : '';
      if (
        !(await confirmAction({
          title: 'Atualizar identidade?',
          message: `Atualizar o nome e o apelido no arquivo base?${externalReview}`,
        }))
      )
        return false;
      await invoke('agent_home_save_core', {
        content: updated,
        expectedHash: current.current_hash,
      });
      setProfile((current) => ({
        ...current,
        name: next.displayName,
        nickname: next.nickname,
      }));
      await refresh();
    }
    return true;
  }
  async function openMemory(name: string) {
    await run(async () => {
      setMemoryDraft(await invoke<string>('agent_memory_read', { name }));
      setMemoryName(name);
    });
  }
  async function saveMemory() {
    await run(async () => {
      if (memoryName.trim().toLocaleLowerCase('pt-BR') === 'estilo.md') {
        throw new Error(
          'Use a seção “Como o OdeKrai fala com você” para editar essas preferências.',
        );
      }
      await invoke('agent_memory_save', { name: memoryName, content: memoryDraft });
      setNames(
        (await invoke<string[]>('agent_memory_list')).filter((name) => name !== 'estilo.md'),
      );
      setNotice('Memória salva.');
    });
  }
  async function deleteMemory() {
    await runConfirmedAction(confirmAction, memoryDeletionConfirmation(memoryName), async () => {
      await run(async () => {
        await invoke('agent_memory_delete', { name: memoryName });
        setNames(
          (await invoke<string[]>('agent_memory_list')).filter((name) => name !== 'estilo.md'),
        );
        setMemoryName('');
        setMemoryDraft('');
      });
    });
  }
  async function deleteAllMemory() {
    if (
      !(await confirmAction({
        title: 'Apagar todas as memórias?',
        message: 'Apagar todos os arquivos de memória? Esta ação não pode ser desfeita.',
        destructive: true,
      }))
    )
      return;
    await run(async () => {
      await invoke('agent_memory_delete_all');
      setNames([]);
      setMemoryName('');
      setMemoryDraft('');
      setPersonalInstructions('');
      setSlang('');
    });
  }
  if (!isTauri()) return <p>A pasta pessoal está disponível no aplicativo Windows.</p>;
  return (
    <div className={embedded ? 'settings-embedded-content' : 'page settings-page'}>
      {!embedded && (
        <PageHeader
          eyebrow="IDENTIDADE LOCAL"
          title="Identidade e memória"
          subtitle="Defina como o OdeKrai conversa com você e edite suas notas locais."
        />
      )}
      {error && (
        <Banner tone="danger" role="alert">
          {error}
        </Banner>
      )}
      {notice && <Banner>{notice}</Banner>}
      {status?.warning && (
        <Banner tone="warning" role="alert">
          {status.warning}
        </Banner>
      )}
      {status === null ? (
        <div>
          <p>Carregando suas preferências…</p>
          {error && (
            <Button
              onClick={() => {
                setError('');
                void refresh().catch((cause) => setError(String(cause)));
              }}
            >
              Tentar de novo
            </Button>
          )}
        </div>
      ) : status.state === 'ausente' ? (
        <Card className="settings-card identity-section" role="status">
          <h2>A pasta do OdeKrai não foi encontrada</h2>
          <p>Não encontrei a pasta do OdeKrai que estava configurada ({status.path}).</p>
          <p>As notas e memórias antigas só voltarão se a pasta anterior for encontrada.</p>
          <div className="identity-actions">
            <Button
              disabled={busy}
              onClick={() => {
                setError('');
                void refresh()
                  .then(() =>
                    setNotice(
                      'Verifiquei novamente o caminho configurado. Se a pasta foi movida, escolha a pasta recuperada manualmente.',
                    ),
                  )
                  .catch((cause) => setError(String(cause)));
              }}
            >
              Procurar de novo
            </Button>
            <Button
              disabled={busy}
              onClick={async () => {
                const selected = await open({
                  directory: true,
                  multiple: false,
                  title: 'Escolha onde criar a pasta do OdeKrai',
                });
                if (typeof selected !== 'string') return;
                await run(async () => {
                  await invoke('agent_home_init', {
                    path: selected,
                    profile: {
                      ...profile,
                      name: profile.name || initialName || 'Usuário',
                      nickname: profile.nickname || initialNickname || 'Usuário',
                    },
                  });
                  await refresh();
                  setNotice(
                    'Nova pasta criada. As notas e memórias antigas só voltarão se a pasta anterior for encontrada.',
                  );
                });
              }}
            >
              Criar uma nova pasta
            </Button>
            <Button
              disabled={busy}
              onClick={async () => {
                const selected = await open({
                  directory: true,
                  multiple: false,
                  title: 'Escolha uma pasta existente do OdeKrai',
                });
                if (typeof selected !== 'string') return;
                await run(async () => {
                  await invoke('agent_home_change_path', { path: selected });
                  await refresh();
                });
              }}
            >
              Escolher uma pasta existente
            </Button>
          </div>
        </Card>
      ) : status.state === 'sem_permissao' || status.state === 'estrutura_incompleta' ? (
        <Card className="settings-card identity-section" role="alert">
          <h2>
            {status.state === 'sem_permissao'
              ? 'Sem permissão para acessar a pasta'
              : 'Estrutura do OdeKrai incompleta'}
          </h2>
          <p>
            {status.message} ({status.path})
          </p>
          <Button
            onClick={() => {
              setError('');
              void refresh().catch((cause) => setError(String(cause)));
            }}
          >
            Tentar de novo
          </Button>
        </Card>
      ) : !status.configured ? (
        <form onSubmit={initialize} className="settings-card identity-wizard">
          <div className="wizard-steps" aria-label={`Passo ${setupStep} de 3`}>
            {['Pasta', 'Você', 'Confirmar'].map((label, index) => (
              <span className={setupStep === index + 1 ? 'active' : ''} key={label}>
                {index + 1}. {label}
              </span>
            ))}
          </div>
          {setupStep === 1 && (
            <section className="identity-wizard-step">
              <h2>Escolha a pasta do OdeKrai</h2>
              <p>Se preferir, use a pasta sugerida.</p>
              <label>
                Pasta
                <TextInput value={path} onChange={(e) => setPath(e.target.value)} required />
              </label>
              <Button
                variant="secondary"
                type="button"
                onClick={async () => {
                  const selected = await open({ directory: true, multiple: false });
                  if (typeof selected === 'string') setPath(selected);
                }}
              >
                Escolher pasta
              </Button>
            </section>
          )}
          {setupStep === 2 && (
            <section className="identity-wizard-step">
              <h2>Como devemos chamar você?</h2>
              <label>
                Seu nome
                <TextInput
                  value={profile.name}
                  onChange={(e) => setProfile({ ...profile, name: e.target.value })}
                  maxLength={60}
                  required
                />
              </label>
              <label>
                Apelido
                <TextInput
                  value={profile.nickname}
                  onChange={(e) => setProfile({ ...profile, nickname: e.target.value })}
                  maxLength={40}
                />
              </label>
              <label>
                Idioma
                <SelectInput
                  value={profile.language}
                  onChange={(e) => setProfile({ ...profile, language: e.target.value })}
                >
                  <option value="pt-BR">Português (Brasil)</option>
                  <option value="en">English</option>
                </SelectInput>
              </label>
              <label>
                Tom
                <SelectInput
                  value={customTone ? 'personalizado' : profile.tone}
                  onChange={(e) => {
                    setCustomTone(e.target.value === 'personalizado');
                    if (e.target.value !== 'personalizado')
                      setProfile({ ...profile, tone: e.target.value });
                  }}
                >
                  {TONE_PRESETS.map((item) => (
                    <option key={item} value={item}>
                      {item[0].toLocaleUpperCase('pt-BR') + item.slice(1)}
                    </option>
                  ))}
                  <option value="personalizado">Personalizado</option>
                </SelectInput>
              </label>
              {customTone && (
                <label>
                  Tom personalizado
                  <TextInput
                    value={profile.tone}
                    onChange={(e) => setProfile({ ...profile, tone: e.target.value })}
                    maxLength={80}
                    required
                  />
                </label>
              )}
            </section>
          )}
          {setupStep === 3 && (
            <section className="identity-wizard-step">
              <h2>Confira antes de salvar</h2>
              <dl className="identity-summary">
                <div>
                  <dt>Pasta</dt>
                  <dd>{path}</dd>
                </div>
                <div>
                  <dt>Nome</dt>
                  <dd>{profile.name}</dd>
                </div>
                <div>
                  <dt>Apelido</dt>
                  <dd>{profile.nickname || 'Não informado'}</dd>
                </div>
                <div>
                  <dt>Idioma</dt>
                  <dd>{profile.language}</dd>
                </div>
                <div>
                  <dt>Tom</dt>
                  <dd>{profile.tone}</dd>
                </div>
              </dl>
            </section>
          )}
          <div className="wizard-actions">
            {setupStep > 1 && (
              <Button type="button" onClick={() => setSetupStep((step) => step - 1)}>
                Voltar
              </Button>
            )}
            <Button
              type="submit"
              variant="primary"
              disabled={busy || (setupStep === 1 && !path.trim())}
            >
              {setupStep === 3 ? 'Salvar identidade' : 'Continuar'}
            </Button>
          </div>
        </form>
      ) : (
        <>
          <div id="identity-profile">
            <Profile
              value={profileValue}
              onChange={onProfileChange}
              onBeforeSave={approveProfileChange}
            />
          </div>
          <Card id="identity-conversation" className="settings-card identity-section">
            <h2>Como o OdeKrai fala com você</h2>
            <div className="identity-fields">
              <label>
                Idioma
                <SelectInput
                  value={profile.language}
                  onChange={(event) => setProfile({ ...profile, language: event.target.value })}
                >
                  <option value="pt-BR">Português (Brasil)</option>
                  <option value="en">English</option>
                </SelectInput>
              </label>
              <label>
                Tom
                <SelectInput
                  value={customTone ? 'personalizado' : profile.tone}
                  onChange={(event) => {
                    setCustomTone(event.target.value === 'personalizado');
                    if (event.target.value !== 'personalizado')
                      setProfile({ ...profile, tone: event.target.value });
                  }}
                >
                  {TONE_PRESETS.map((item) => (
                    <option key={item} value={item}>
                      {item}
                    </option>
                  ))}
                  <option value="personalizado">Personalizado</option>
                </SelectInput>
              </label>
              {customTone && (
                <label>
                  Tom personalizado
                  <TextInput
                    value={profile.tone}
                    maxLength={80}
                    onChange={(event) => setProfile({ ...profile, tone: event.target.value })}
                  />
                </label>
              )}
            </div>
            <label>
              Instruções pessoais
              <textarea
                value={personalInstructions}
                onChange={(event) => setPersonalInstructions(event.target.value)}
                maxLength={400}
                rows={3}
                placeholder="Ex.: prefiro respostas objetivas."
              />
            </label>
            <label>
              Gírias e expressões que você usa
              <textarea
                value={slang}
                onChange={(event) => setSlang(event.target.value)}
                maxLength={400}
                rows={3}
                placeholder="Ex.: palavras e expressões que fazem parte do seu jeito de falar."
              />
            </label>
            <p>Essas notas são manuais. Não inclua senhas, tokens ou outros dados secretos.</p>
            <Button
              variant="primary"
              disabled={busy}
              onClick={() => void saveConversationPreferences()}
            >
              Salvar preferências
            </Button>
          </Card>
          <Card id="identity-memory" className="settings-card identity-section">
            <h2>Memória</h2>
            <p>Suas notas ficam visíveis para você e podem ser alteradas a qualquer momento.</p>
            <div className="memory-file-list">
              {names.map((name) => (
                <Button key={name} type="button" onClick={() => void openMemory(name)}>
                  {name}
                </Button>
              ))}
            </div>
            <Button
              type="button"
              onClick={() => {
                setMemoryName('');
                setMemoryDraft('');
              }}
            >
              Adicionar nota
            </Button>
            <div className="identity-fields">
              <label>
                Nome da nota
                <TextInput
                  value={memoryName}
                  onChange={(event) => setMemoryName(event.target.value)}
                  maxLength={64}
                  placeholder="minha-nota.md"
                />
              </label>
            </div>
            <label>
              Conteúdo
              <textarea
                aria-label="Conteúdo da memória"
                value={memoryDraft}
                onChange={(event) => setMemoryDraft(event.target.value)}
                maxLength={8192}
                rows={6}
              />
            </label>
            <div className="identity-actions">
              <Button
                variant="primary"
                disabled={busy || !memoryName}
                onClick={() => void saveMemory()}
              >
                Salvar nota
              </Button>
              <Button
                disabled={busy || !names.includes(memoryName)}
                onClick={() => void deleteMemory()}
              >
                Apagar nota
              </Button>
              <Button disabled={busy || names.length === 0} onClick={() => void deleteAllMemory()}>
                Apagar todas
              </Button>
            </div>
          </Card>
          <Card className="settings-card identity-section">
            <h2>Pasta do OdeKrai</h2>
            <p className="identity-path">{status.path}</p>
            <p>
              Ao escolher outra pasta do OdeKrai, confira e confirme o arquivo base antes de usá-lo.
            </p>
            <div className="identity-actions">
              <Button
                onClick={() =>
                  void invoke('agent_home_open_folder').catch((cause) => setError(String(cause)))
                }
              >
                Abrir pasta
              </Button>
              <Button
                onClick={async () => {
                  const selected = await open({ directory: true, multiple: false });
                  if (typeof selected !== 'string') return;
                  await run(async () => {
                    const changed = await invoke<HomeStatus>('agent_home_change_path', {
                      path: selected,
                    });
                    setStatus(changed);
                    setPath(changed.path);
                    await refresh();
                    setNotice(
                      'Pasta alterada. Confira e confirme o arquivo base para continuar usando a identidade.',
                    );
                  });
                }}
              >
                Trocar
              </Button>
            </div>
          </Card>
          <Card className="settings-card identity-section">
            <h2>Arquivo base</h2>
            <p>
              Estado:{' '}
              <strong>
                {!core
                  ? 'Verificando'
                  : core.changed_externally
                    ? 'Alterado fora do app'
                    : 'Aprovado'}
              </strong>
            </p>
            {core?.changed_externally && (
              <>
                <Banner tone="warning">
                  O arquivo mudou. Confira as diferenças e confirme antes de continuar.
                </Banner>
                <details open>
                  <summary>Ver diferenças</summary>
                  <pre className="identity-diff">{core.diff}</pre>
                </details>
              </>
            )}
            <Button
              variant="primary"
              onClick={() => void saveCore()}
              disabled={busy || (!core?.changed_externally && coreDraft === core?.content)}
            >
              Confirmar arquivo base
            </Button>
            <details>
              <summary>Avançado</summary>
              <p>Use este editor somente se quiser revisar o conteúdo completo do arquivo.</p>
              <textarea
                aria-label="Conteúdo avançado do arquivo base"
                value={coreDraft}
                onChange={(event) => setCoreDraft(event.target.value)}
                rows={14}
                maxLength={16384}
              />
            </details>
          </Card>
        </>
      )}
    </div>
  );
}
