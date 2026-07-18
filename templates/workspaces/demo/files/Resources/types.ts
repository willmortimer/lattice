/** Shared types for demo Resources — not imported by the app. */
export type ContactStatus = "Lead" | "Active" | "Nurture" | "Churned" | "Partner";

export interface CrmContactSeed {
  name: string;
  email: string;
  company: string;
  due_date: string | null;
  status: ContactStatus;
  notes: string;
}

export function formatDueLabel(isoDate: string | null): string {
  if (!isoDate) return "No date";
  return new Date(isoDate).toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
    year: "numeric",
  });
}
