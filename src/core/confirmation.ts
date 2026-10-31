export type ConfirmationRequest = {
  title: string;
  message: string;
  destructive?: boolean;
};

export type ConfirmationHandler = (request: ConfirmationRequest) => Promise<boolean>;

export async function runConfirmedAction(
  confirm: ConfirmationHandler,
  request: ConfirmationRequest,
  action: () => void | Promise<void>,
): Promise<boolean> {
  if (!(await confirm(request))) return false;
  await action();
  return true;
}

export function memoryDeletionConfirmation(memoryName: string): ConfirmationRequest {
  return {
    title: 'Apagar memória?',
    message: `Apagar ${memoryName}?`,
    destructive: true,
  };
}
