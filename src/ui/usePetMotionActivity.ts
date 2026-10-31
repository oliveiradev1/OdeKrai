import { useSyncExternalStore } from 'react';
import { isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getAllWindows, getCurrentWindow } from '@tauri-apps/api/window';
import {
  PET_UNFOCUSED_PAUSE_DELAY_MS,
  shouldPausePetAnimation,
  type PetWindowActivity,
} from '../core/pet-motion';

const subscribers = new Set<() => void>();
let activity: PetWindowActivity = {
  documentVisible: true,
  windowVisible: true,
  minimized: false,
  focused: true,
  unfocusedForMs: 0,
};
let unfocusedAt: number | null = null;
let focusTimer: number | undefined;
let active = true;
let performanceMode =
  typeof document !== 'undefined' && document.documentElement.dataset.performance === 'on';
let detachListeners: (() => void) | null = null;

function publish() {
  activity = {
    ...activity,
    unfocusedForMs: unfocusedAt === null ? 0 : Math.max(0, Date.now() - unfocusedAt),
  };
  const nextActive = !shouldPausePetAnimation(activity);
  const nextPerformanceMode =
    typeof document !== 'undefined' && document.documentElement.dataset.performance === 'on';
  if (active === nextActive && performanceMode === nextPerformanceMode) return;
  active = nextActive;
  performanceMode = nextPerformanceMode;
  subscribers.forEach((subscriber) => subscriber());
}

function subscribe(subscriber: () => void): () => void {
  subscribers.add(subscriber);
  if (subscribers.size === 1) attachListeners();
  return () => {
    subscribers.delete(subscriber);
    if (subscribers.size === 0) {
      detachListeners?.();
      detachListeners = null;
    }
  };
}

function attachListeners() {
  let disposed = false;
  const removers: Array<() => void> = [];
  const performanceObserver = new MutationObserver(publish);
  performanceObserver.observe(document.documentElement, {
    attributes: true,
    attributeFilter: ['data-performance'],
  });
  removers.push(() => performanceObserver.disconnect());
  const addAsyncRemover = (promise: Promise<() => void>) => {
    void promise
      .then((remove) => {
        if (disposed) remove();
        else removers.push(remove);
      })
      .catch(() =>
        console.warn('[OdeKrai diagnóstico] Falha ao acompanhar a atividade da janela do mascote.'),
      );
  };
  let syncWindowState = async () => undefined;
  const handleDocumentVisibility = () => {
    activity = { ...activity, documentVisible: document.visibilityState === 'visible' };
    publish();
    if (activity.documentVisible) void syncWindowState();
  };
  document.addEventListener('visibilitychange', handleDocumentVisibility);
  removers.push(() => document.removeEventListener('visibilitychange', handleDocumentVisibility));

  if (isTauri()) {
    const currentWindow = getCurrentWindow();
    const focusWindowPromise =
      currentWindow.label === 'pet'
        ? getAllWindows()
            .then((windows) => windows.find((item) => item.label === 'main') ?? currentWindow)
            .catch(() => currentWindow)
        : Promise.resolve(currentWindow);
    const scheduleUnfocusedPause = () => {
      window.clearTimeout(focusTimer);
      if (activity.focused) return;
      if (!activity.windowVisible || activity.minimized) {
        publish();
        return;
      }
      unfocusedAt ??= Date.now();
      const remaining = Math.max(0, PET_UNFOCUSED_PAUSE_DELAY_MS - (Date.now() - unfocusedAt));
      focusTimer = window.setTimeout(() => {
        if (!activity.focused) publish();
      }, remaining);
    };
    syncWindowState = async () => {
      try {
        const focusWindow = await focusWindowPromise;
        const [visible, minimized, focused] = await Promise.all([
          currentWindow.isVisible(),
          currentWindow.isMinimized(),
          focusWindow.isFocused(),
        ]);
        if (disposed) return;
        activity = { ...activity, windowVisible: visible, minimized, focused };
        if (focused) {
          unfocusedAt = null;
          window.clearTimeout(focusTimer);
        } else if (unfocusedAt === null) {
          unfocusedAt = Date.now();
        }
        publish();
        if (!focused) scheduleUnfocusedPause();
      } catch {
        // A consulta pode falhar durante o encerramento da janela; visibilitychange segue ativo.
      }
    };
    void syncWindowState();
    addAsyncRemover(
      focusWindowPromise.then((focusWindow) =>
        focusWindow.onFocusChanged(({ payload: focused }) => {
          activity = { ...activity, focused };
          window.clearTimeout(focusTimer);
          if (focused) {
            unfocusedAt = null;
            void syncWindowState();
          } else {
            unfocusedAt = Date.now();
            void syncWindowState();
          }
          publish();
        }),
      ),
    );
    if (currentWindow.label === 'pet') {
      addAsyncRemover(
        listen<boolean>('pet-window-visibility', ({ payload }) => {
          activity = { ...activity, windowVisible: payload };
          publish();
        }),
      );
    }
  }

  publish();
  detachListeners = () => {
    disposed = true;
    window.clearTimeout(focusTimer);
    removers.splice(0).forEach((remove) => remove());
  };
}

export function usePetMotionActive(): boolean {
  return useSyncExternalStore(
    subscribe,
    () => active,
    () => true,
  );
}

export function usePetPerformanceMode(): boolean {
  return useSyncExternalStore(
    subscribe,
    () => performanceMode,
    () => false,
  );
}
