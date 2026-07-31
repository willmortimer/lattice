import { Button as BaseButton } from "@base-ui/react/button";
import { Checkbox } from "@base-ui/react/checkbox";
import { Dialog } from "@base-ui/react/dialog";
import { Menu } from "@base-ui/react/menu";
import { Popover } from "@base-ui/react/popover";
import { Radio } from "@base-ui/react/radio";
import { RadioGroup } from "@base-ui/react/radio-group";
import { Tabs } from "@base-ui/react/tabs";
import { Tooltip } from "@base-ui/react/tooltip";
import type { ComponentProps, ReactNode } from "react";

import "./styles.css";

function classes(...values: Array<string | false | null | undefined>) {
  return values.filter(Boolean).join(" ");
}

export type ButtonProps = Omit<ComponentProps<typeof BaseButton>, "className"> & {
  className?: string;
  variant?: "primary" | "secondary" | "ghost" | "danger";
  size?: "sm" | "md";
};

export function Button({
  className,
  variant = "secondary",
  size = "md",
  ...props
}: ButtonProps) {
  return (
    <BaseButton
      className={classes("ltui-button", `ltui-button-${variant}`, `ltui-button-${size}`, className)}
      {...props}
    />
  );
}

export function IconButton({
  label,
  tooltip,
  className,
  ...props
}: Omit<ButtonProps, "children"> & {
  label: string;
  /** Pass `false` to skip the tooltip portal (prefer for dense virtualized rows). */
  tooltip?: string | false;
  children: ReactNode;
}) {
  const button = (
    <Button
      aria-label={label}
      className={classes("ltui-icon-button", className)}
      variant="ghost"
      size="sm"
      {...props}
    />
  );
  if (tooltip === false) {
    return button;
  }
  const tip = tooltip ?? label;
  return (
    <Tooltip.Root>
      <Tooltip.Trigger render={button} />
      <Tooltip.Portal>
        <Tooltip.Positioner sideOffset={7}>
          <Tooltip.Popup className="ltui-tooltip">{tip}</Tooltip.Popup>
        </Tooltip.Positioner>
      </Tooltip.Portal>
    </Tooltip.Root>
  );
}

export function TooltipProvider({ children }: { children: ReactNode }) {
  return <Tooltip.Provider delay={500}>{children}</Tooltip.Provider>;
}

export const MenuRoot = Menu.Root;
export const MenuTrigger = Menu.Trigger;
export const MenuPortal = Menu.Portal;
export const MenuPositioner = Menu.Positioner;
export const MenuPopup = Menu.Popup;
export const MenuItem = Menu.Item;
export const MenuSeparator = Menu.Separator;

export const PopoverRoot = Popover.Root;
export const PopoverTrigger = Popover.Trigger;
export const PopoverPortal = Popover.Portal;
export const PopoverPositioner = Popover.Positioner;
export const PopoverPopup = Popover.Popup;
export const PopoverClose = Popover.Close;

export const TabsRoot = Tabs.Root;

export type TabsListProps = Omit<ComponentProps<typeof Tabs.List>, "className"> & {
  className?: string;
};

/** Quiet segmented tab list — sunken track, raised active segment. */
export function TabsList({ className, ...props }: TabsListProps) {
  return <Tabs.List className={classes("ltui-tabs-list", className)} {...props} />;
}

export type TabsTabProps = Omit<ComponentProps<typeof Tabs.Tab>, "className"> & {
  className?: string;
};

export function TabsTab({ className, ...props }: TabsTabProps) {
  return <Tabs.Tab className={classes("ltui-tabs-tab", className)} {...props} />;
}

export type TabsPanelProps = Omit<ComponentProps<typeof Tabs.Panel>, "className"> & {
  className?: string;
};

export function TabsPanel({ className, ...props }: TabsPanelProps) {
  return <Tabs.Panel className={classes("ltui-tabs-panel", className)} {...props} />;
}

export const TabsIndicator = Tabs.Indicator;

export interface SurfaceHeaderProps {
  /** Leading mark (the desktop app passes its KindMark here). */
  icon?: ReactNode;
  title: ReactNode;
  /** Short code-ish subtitle (path, entrypoint, …) rendered in mono. */
  subtitle?: ReactNode;
  /** Right-aligned metadata (pills, counts) before the actions. */
  meta?: ReactNode;
  actions?: ReactNode;
  className?: string;
}

/**
 * Compact resource-surface header: icon, title + one-line subtitle on the
 * left, metadata and actions on the right. The shell breadcrumb already
 * shows the full path, so this stays a single quiet row.
 */
export function SurfaceHeader({
  icon,
  title,
  subtitle,
  meta,
  actions,
  className,
}: SurfaceHeaderProps) {
  return (
    <header className={classes("ltui-surface-header", className)}>
      {icon ? (
        <span className="ltui-surface-header-icon" aria-hidden>
          {icon}
        </span>
      ) : null}
      <div className="ltui-surface-header-text">
        <h1 className="ltui-surface-header-title">{title}</h1>
        {subtitle ? <p className="ltui-surface-header-subtitle">{subtitle}</p> : null}
      </div>
      {meta ? <div className="ltui-surface-header-meta">{meta}</div> : null}
      {actions ? <div className="ltui-surface-header-actions">{actions}</div> : null}
    </header>
  );
}

export const DialogRoot = Dialog.Root;
export const DialogPortal = Dialog.Portal;
export const DialogBackdrop = Dialog.Backdrop;
export const DialogPopup = Dialog.Popup;
export const DialogTitle = Dialog.Title;
export const DialogDescription = Dialog.Description;
export const DialogClose = Dialog.Close;

export const RadioGroupRoot = RadioGroup;
export const RadioItem = Radio.Root;
export const RadioIndicator = Radio.Indicator;

export const CheckboxRoot = Checkbox.Root;
export const CheckboxIndicator = Checkbox.Indicator;
