import { Button as BaseButton } from "@base-ui/react/button";
import { Menu } from "@base-ui/react/menu";
import { Popover } from "@base-ui/react/popover";
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
  tooltip = label,
  className,
  ...props
}: Omit<ButtonProps, "children"> & {
  label: string;
  tooltip?: string;
  children: ReactNode;
}) {
  return (
    <Tooltip.Root>
      <Tooltip.Trigger
        render={
          <Button
            aria-label={label}
            className={classes("ltui-icon-button", className)}
            variant="ghost"
            size="sm"
            {...props}
          />
        }
      />
      <Tooltip.Portal>
        <Tooltip.Positioner sideOffset={7}>
          <Tooltip.Popup className="ltui-tooltip">{tooltip}</Tooltip.Popup>
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
export const TabsList = Tabs.List;
export const TabsTab = Tabs.Tab;
export const TabsPanel = Tabs.Panel;
export const TabsIndicator = Tabs.Indicator;
