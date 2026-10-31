export function shouldAutomaticallyCheckForUpdates(enabled: boolean, nativeApp: boolean): boolean {
  return enabled && nativeApp;
}
