export async function runOnceWhilePending<T>(
  slot: { current: Promise<T> | null },
  run: () => Promise<T>,
): Promise<T> {
  if (slot.current) return slot.current;

  const request = run();
  slot.current = request;
  try {
    return await request;
  } finally {
    if (slot.current === request) slot.current = null;
  }
}
