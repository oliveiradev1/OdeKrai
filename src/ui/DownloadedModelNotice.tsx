export function DownloadedModelNotice() {
  return (
    <div className="downloaded-model-notice">
      <span>Estado: Baixado. Ainda não é usado pelo OdeKrai.</span>
      <details>
        <summary>Como usar</summary>
        <ol>
          <li>Instale um servidor compatível com a API OpenAI, como Ollama ou llama.cpp.</li>
          <li>
            Inicie o servidor apontando para o arquivo GGUF baixado.
            <p>
              Com Ollama, crie um arquivo Modelfile com a linha abaixo e, na mesma pasta, execute os
              comandos. Troque o caminho pelo caminho absoluto do arquivo GGUF no Windows:
            </p>
            <pre>
              {'FROM /caminho/absoluto/modelo.gguf\nollama create meu-modelo\nollama serve'}
            </pre>
            <p>Ollama usa 127.0.0.1:11434 por padrão.</p>
            <p>Com llama.cpp no Windows, execute:</p>
            <pre>{'llama-server.exe -m "C:\\caminho\\para\\modelo.gguf" -c 2048'}</pre>
            <p>llama-server usa 127.0.0.1:9931 por padrão.</p>
          </li>
          <li>
            Volte a Conectores de IA &gt; Gratuitos, informe outra porta se necessário e clique em
            “Procurar IA local”.
          </li>
        </ol>
      </details>
    </div>
  );
}
