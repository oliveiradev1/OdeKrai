export function CodeDelegationSettings() {
  return (
    <section className="settings-panel settings-section" aria-labelledby="code-delegation-heading">
      <h2 id="code-delegation-heading">Ferramenta de código</h2>
      <p className="settings-description" role="status">
        A delegação para Codex CLI e Claude Code está temporariamente desativada. O worktree não é
        um sandbox: sem isolamento verificado pelo sistema operacional, uma ferramenta poderia
        acessar arquivos, rede e credenciais da sua conta. A função será reavaliada quando houver
        uma barreira efetiva de isolamento.
      </p>
    </section>
  );
}
