export const PET_UNFOCUSED_PAUSE_DELAY_MS = 2500;

export type PetWindowActivity = {
  documentVisible: boolean;
  windowVisible: boolean;
  minimized: boolean;
  focused: boolean;
  unfocusedForMs: number;
};

export function shouldPausePetAnimation(activity: PetWindowActivity): boolean {
  return (
    !activity.documentVisible ||
    !activity.windowVisible ||
    activity.minimized ||
    (!activity.focused && activity.unfocusedForMs >= PET_UNFOCUSED_PAUSE_DELAY_MS)
  );
}

export function petAnimationPlayState(
  paused: boolean,
  motionDisabled: boolean,
): 'paused' | 'running' {
  return paused || motionDisabled ? 'paused' : 'running';
}
