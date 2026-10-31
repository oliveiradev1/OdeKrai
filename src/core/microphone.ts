export function microphoneClickAction(enabled: boolean): 'record' | 'activate' {
  return enabled ? 'record' : 'activate';
}

export async function activateMicrophone<T>(
  persistPreference: () => void | Promise<void>,
  requestPermission: () => Promise<MediaStream>,
  beginRecording: (stream: MediaStream) => T | Promise<T>,
): Promise<T> {
  await persistPreference();
  const stream = await requestPermission();
  return beginRecording(stream);
}

export function microphonePermissionError(error: unknown): string {
  const name = error instanceof DOMException ? error.name : '';
  if (name === 'NotFoundError' || name === 'DevicesNotFoundError') {
    return 'Nenhum microfone foi encontrado. Conecte ou habilite um dispositivo de entrada.';
  }
  if (name === 'NotAllowedError' || name === 'SecurityError') {
    return 'A permissão do microfone foi negada. Em Configurações do Windows → Privacidade e segurança → Microfone, habilite o acesso ao microfone e o acesso para aplicativos da área de trabalho.';
  }
  return 'Não foi possível iniciar a gravação. Verifique se o microfone está conectado e disponível.';
}
