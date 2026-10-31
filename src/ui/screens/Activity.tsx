import { SecurityCenter } from '../SecurityCenter';
import type { AuditEntry } from '../SecurityCenter';
export function ActivityScreen({
  sharedAudit,
}: {
  sharedAudit?: {
    entries: AuditEntry[];
    verified: boolean | null;
    loading: boolean;
    unavailable?: boolean;
  };
}) {
  return <SecurityCenter page="activity" sharedAudit={sharedAudit} />;
}
