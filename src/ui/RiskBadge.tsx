import {
  Code,
  Eye,
  PaperPlaneRight,
  PencilSimple,
  Question,
  Warning,
  EyeSlash,
  type Icon as PhosphorIcon,
} from '@phosphor-icons/react';
import type { SkillRisk } from '../core/skills-catalog';

export type RiskLevel = SkillRisk;

const risks: Record<RiskLevel, { label: string; icon: PhosphorIcon }> = {
  leitura: { label: 'Leitura', icon: Eye },
  escrita_reversivel: { label: 'Escrita reversível', icon: PencilSimple },
  comunicacao_externa: { label: 'Comunicação externa', icon: PaperPlaneRight },
  destrutivo_financeiro: { label: 'Destrutivo ou financeiro', icon: Warning },
  execucao_de_codigo: { label: 'Execução de código (alto)', icon: Code },
  captura_sensivel: { label: 'Captura sensível', icon: EyeSlash },
};

function normalizeRisk(value: string): RiskLevel | null {
  const normalized = value
    .normalize('NFD')
    .replace(/[\u0300-\u036f]/g, '')
    .toLocaleLowerCase('pt-BR')
    .replace(/\s+/g, ' ')
    .trim();
  const aliases: Record<string, RiskLevel> = {
    leitura: 'leitura',
    escrita_reversivel: 'escrita_reversivel',
    'escrita reversivel': 'escrita_reversivel',
    comunicacao_externa: 'comunicacao_externa',
    'comunicacao externa': 'comunicacao_externa',
    destrutivo_financeiro: 'destrutivo_financeiro',
    'destrutivo ou financeiro': 'destrutivo_financeiro',
    execucao_de_codigo: 'execucao_de_codigo',
    'execucao de codigo (alto)': 'execucao_de_codigo',
    captura_sensivel: 'captura_sensivel',
    'captura sensivel': 'captura_sensivel',
  };
  return aliases[normalized] ?? null;
}

export function RiskBadge({
  risk,
  className = '',
}: {
  risk: RiskLevel | string;
  className?: string;
}) {
  const id = normalizeRisk(risk);
  const config = id ? risks[id] : { label: 'Risco não identificado', icon: Question };
  const Icon = config.icon;
  return (
    <span
      className={`risk-badge ${id ? `risk-badge--${id}` : 'risk-badge--unknown'} ${className}`.trim()}
      aria-label={`Risco: ${config.label}`}
      title={`Risco: ${config.label}`}
    >
      <Icon size={14} weight="light" aria-hidden="true" focusable="false" />
      <span>{config.label}</span>
    </span>
  );
}
