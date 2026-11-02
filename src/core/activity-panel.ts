export type ActivityAuditSummary = {
  id: string;
  timestamp: number;
  tool: string;
  status: string;
  decision: string;
};

export type UpcomingReminder = {
  title: string;
  whenLocal: string;
  status: 'pending' | 'notification_error' | string;
};

export function activityPanelTabIndex(
  key: string,
  shiftKey: boolean,
  index: number,
  count: number,
) {
  if (key === 'Escape') return -1;
  if (key !== 'Tab' || count === 0) return null;
  if (shiftKey && index === 0) return count - 1;
  if (!shiftKey && index === count - 1) return 0;
  return null;
}

export function toggleActivityPanel(open: boolean) {
  return !open;
}

export function shouldLoadPanelAudit(open: boolean) {
  return open;
}

export function focusActivityPanel(
  trigger: { focus: () => void },
  initialFocus: { focus: () => void },
) {
  initialFocus.focus();
  return () => trigger.focus();
}
