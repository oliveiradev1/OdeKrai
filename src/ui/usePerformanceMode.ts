import { useEffect, useMemo, useState } from 'react';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { emit } from '@tauri-apps/api/event';
import { isHardwareProfile } from '../core/hardware-profile';
import {
  evaluatePerformanceMode,
  type PerformanceMode,
  type PerformanceModeStatus,
} from '../core/performance';

export type PerformanceModeView = PerformanceModeStatus & { loading: boolean };

export function usePerformanceMode(mode: PerformanceMode): PerformanceModeView {
  const [prefersReducedMotion, setPrefersReducedMotion] = useState(
    () =>
      typeof window !== 'undefined' &&
      window.matchMedia('(prefers-reduced-motion: reduce)').matches,
  );
  const [hardware, setHardware] = useState<{ total_ram_bytes: number; cpu_cores: number } | null>(
    null,
  );
  const [hardwareLoading, setHardwareLoading] = useState(mode === 'automatic' && isTauri());

  useEffect(() => {
    const preference = window.matchMedia('(prefers-reduced-motion: reduce)');
    const update = () => setPrefersReducedMotion(preference.matches);
    update();
    preference.addEventListener('change', update);
    return () => preference.removeEventListener('change', update);
  }, []);

  useEffect(() => {
    if (mode !== 'automatic' || !isTauri()) {
      setHardware(null);
      setHardwareLoading(false);
      return;
    }
    let active = true;
    setHardwareLoading(true);
    void invoke<unknown>('get_hardware_profile')
      .then((profile) => {
        if (!active) return;
        if (!isHardwareProfile(profile)) throw new TypeError('Perfil de hardware inválido.');
        setHardware({
          total_ram_bytes: profile.total_ram_bytes,
          cpu_cores: profile.cpu_cores,
        });
      })
      .catch(() => {
        if (active) setHardware(null);
      })
      .finally(() => {
        if (active) setHardwareLoading(false);
      });
    return () => {
      active = false;
    };
  }, [mode]);

  const result = useMemo(
    () => evaluatePerformanceMode(mode, prefersReducedMotion, hardware),
    [hardware, mode, prefersReducedMotion],
  );

  useEffect(() => {
    document.documentElement.dataset.performance = result.enabled ? 'on' : 'off';
  }, [result.enabled]);

  useEffect(() => {
    if (!isTauri()) return;
    void emit('performance-mode-updated', result).catch(() => undefined);
  }, [result.enabled, result.reason]);

  return { ...result, loading: mode === 'automatic' && hardwareLoading };
}
