export async function submitSecretForm(
  event: { preventDefault: () => void },
  save: () => Promise<boolean>,
  clear: () => void,
): Promise<void> {
  event.preventDefault();
  if (await save()) clear();
}
