import type { ReactNode } from "react";
import { SectionCard } from "@/components/ui";

/** Web-cabinet style section on top of SectionCard (same props as the cabinet's Section). */
export function Section({
  title,
  description,
  actions,
  children,
  danger,
  disablePadding,
}: {
  id?: string;
  title: ReactNode;
  description?: ReactNode;
  actions?: ReactNode;
  children?: ReactNode;
  danger?: boolean;
  disablePadding?: boolean;
}) {
  return (
    <SectionCard
      title={title}
      description={description}
      action={actions}
      tone={danger ? "warning" : undefined}
      sx={disablePadding ? { "& > :last-child": { p: 0 } } : undefined}
    >
      {children}
    </SectionCard>
  );
}
