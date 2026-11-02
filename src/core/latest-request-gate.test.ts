import { describe, expect, it } from 'vitest';
import { LatestRequestGate } from './latest-request-gate';

describe('LatestRequestGate', () => {
  it('ignores a late conversation load after another conversation is selected', async () => {
    const gate = new LatestRequestGate();
    let resolveOld!: (value: string) => void;
    const oldLoad = new Promise<string>((resolve) => {
      resolveOld = resolve;
    });
    let displayed = '';
    const oldRequest = gate.begin();
    const oldResult = oldLoad.then((value) => {
      if (gate.isCurrent(oldRequest)) displayed = value;
    });

    gate.begin();
    resolveOld('resposta da conversa antiga');
    await oldResult;

    expect(displayed).toBe('');
  });

  it('ignores a late load after starting a new conversation', () => {
    const gate = new LatestRequestGate();
    const load = gate.begin();
    gate.invalidate();
    expect(gate.isCurrent(load)).toBe(false);
  });
});
