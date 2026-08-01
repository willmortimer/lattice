import { useSettingsHighlight } from "./settingsHighlight";

export function SettingRow({
  settingId,
  title,
  description,
  children,
}: {
  settingId?: string;
  title: string;
  description: string;
  children: React.ReactNode;
}) {
  const highlighted = useSettingsHighlight(settingId);

  return (
    <div
      className={`setting-row${highlighted ? " setting-row-highlight" : ""}`}
      data-setting-id={settingId}
    >
      <div>
        <strong>{title}</strong>
        <span>{description}</span>
      </div>
      <div className="setting-control">{children}</div>
    </div>
  );
}
