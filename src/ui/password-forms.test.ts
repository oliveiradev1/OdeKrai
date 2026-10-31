import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

describe('formulários de credenciais', () => {
  it('mantém campos de senha dentro de formulários semânticos', () => {
    const sources = [
      readFileSync(new URL('./DiscordWebhooksSettings.tsx', import.meta.url), 'utf8'),
      readFileSync(new URL('./PrivacyConfigurations.tsx', import.meta.url), 'utf8'),
      readFileSync(new URL('./SkillsScreen.tsx', import.meta.url), 'utf8'),
      readFileSync(new URL('./ProviderCatalog.tsx', import.meta.url), 'utf8'),
      readFileSync(new URL('./CloudSettings.tsx', import.meta.url), 'utf8'),
    ];
    expect(sources[0]).toMatch(/<form[\s\S]*?type="password"[\s\S]*?<\/form>/);
    expect(sources[1]).toMatch(
      /className="mcp-server-form"[\s\S]*?className="mcp-settings-grid"[\s\S]*?type="password"[\s\S]*?type="submit"/,
    );
    expect(sources[1]).toMatch(
      /className="mcp-env-field"[\s\S]*?type="password"[\s\S]*?type="submit"/,
    );
    expect(sources[2]).toMatch(
      /className="skills-google-fields"[\s\S]*?type="password"[\s\S]*?type="submit"/,
    );
    expect(sources[3]).toMatch(
      /className="provider-form"[\s\S]*?type=\{showKey \? 'text' : 'password'\}[\s\S]*?type="submit"/,
    );
    for (const source of sources.slice(0, 3)) expect(source).toContain('autoComplete="off"');
    expect(sources[3]).toContain('autoComplete="new-password"');
    expect(sources[4]).toContain('autoComplete="off"');
    expect(sources[4]).toContain('className="cloud-token-entry"');
  });
});
