import { useState } from 'react';
import {
  Badge,
  Banner,
  Button,
  Card,
  EmptyState,
  Kbd,
  ListItem,
  SelectInput,
  SelectableCard,
  Skeleton,
  StatusBadge,
  Tabs,
  TextInput,
  Toast,
  Toggle,
  Tooltip,
} from './DesignSystem';

const tabItems = [
  { id: 'visao', label: 'Visão geral', content: <p>Painel da aba selecionada.</p> },
  { id: 'detalhes', label: 'Detalhes', content: <p>Conteúdo alternativo.</p> },
  {
    id: 'indisponivel',
    label: 'Indisponível',
    content: <p>Estado desabilitado.</p>,
    disabled: true,
  },
] as const;

export function DesignSystemGallery() {
  const [tab, setTab] = useState('visao');
  const [toggle, setToggle] = useState(true);
  const [toastVisible, setToastVisible] = useState(true);
  const [selected, setSelected] = useState(false);
  return (
    <main className="ui-gallery">
      <header>
        <span className="eyebrow">ODEKRAI · DESENVOLVIMENTO</span>
        <h1>Galeria do sistema visual</h1>
        <p>Prévia local dos componentes base, estados e interações acessíveis.</p>
      </header>
      <section aria-labelledby="gallery-actions">
        <h2 id="gallery-actions">Ações e entradas</h2>
        <div className="ui-gallery-row">
          <Button variant="primary">Primário</Button>
          <Button variant="secondary">Secundário</Button>
          <Button variant="ghost">Discreto</Button>
          <Button variant="danger">Perigo</Button>
          <TextInput aria-label="Campo de texto" placeholder="Campo de texto" />
          <SelectInput aria-label="Seletor">
            <option>Opção</option>
          </SelectInput>
          <Toggle label="Ativar opção" checked={toggle} onClick={() => setToggle(!toggle)} />
          <SelectableCard selected={selected} onClick={() => setSelected(!selected)}>
            Cartão selecionável
          </SelectableCard>
        </div>
      </section>
      <section aria-labelledby="gallery-surfaces">
        <h2 id="gallery-surfaces">Superfícies e estados</h2>
        <div className="ui-gallery-grid">
          <Card>
            <strong>Card</strong>
            <p>Superfície com token de borda e fundo.</p>
          </Card>
          <div className="ui-gallery-stack">
            <Badge>Neutro</Badge>
            <Badge tone="success">Sucesso</Badge>
            <Badge tone="warning">Atenção</Badge>
            <Badge tone="danger">Erro</Badge>
          </div>
          <div className="ui-gallery-stack">
            {(['ativa', 'desativada', 'pendente', 'erro', 'sucesso'] as const).map((state) => (
              <StatusBadge key={state} state={state} />
            ))}
          </div>
          <div className="ui-gallery-stack">
            <Banner tone="info">Informação</Banner>
            <Banner tone="warning">Atenção</Banner>
            <Banner tone="danger" role="alert">
              Falha que requer atenção
            </Banner>
          </div>
        </div>
      </section>
      <section aria-labelledby="gallery-tabs">
        <h2 id="gallery-tabs">Navegação e retorno</h2>
        <Tabs items={tabItems} selectedId={tab} onSelect={setTab} label="Abas de demonstração" />
        {toastVisible ? (
          <Toast
            action={{ onClick: () => setToastVisible(false) }}
            onDismiss={() => setToastVisible(false)}
          >
            Alteração concluída. Você pode desfazê-la.
          </Toast>
        ) : (
          <Button onClick={() => setToastVisible(true)}>Mostrar aviso</Button>
        )}
        <Tooltip content="Uma dica aparece ao focar ou apontar.">
          <Button>Foque para ver a dica</Button>
        </Tooltip>
        <p>
          Atalho: <Kbd>Ctrl</Kbd> + <Kbd>K</Kbd>
        </p>
      </section>
      <section aria-labelledby="gallery-feedback">
        <h2 id="gallery-feedback">Carregamento, lista e vazio</h2>
        <div className="ui-gallery-stack" aria-label="Exemplos de carregamento">
          <Skeleton width="40%" />
          <Skeleton width="70%" height="32px" />
        </div>
        <ul className="ui-gallery-list">
          <ListItem
            leading="●"
            description="Linha auxiliar"
            trailing={<StatusBadge state="ativa" />}
          >
            Item de lista
          </ListItem>
        </ul>
        <EmptyState
          icon={<span aria-hidden="true">◎</span>}
          title="Ainda não há itens"
          description="Quando houver conteúdo, ele aparecerá nesta área."
          action={{ label: 'Criar item', onClick: () => undefined }}
        />
      </section>
    </main>
  );
}
