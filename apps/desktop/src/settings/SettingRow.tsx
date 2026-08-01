import { useSettingsHighlight } from "./settingsHighlight";

export function SettingRow({
  settingId,
  title,
  description,
  inlineStatus,
  inlineError,
  children,
}: {
  settingId?: string;
  title: string;
  description: string;
  inlineStatus?: string | null;
  inlineError?: string | null;
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
      <div className="setting-control">
        {children}
        {inlineStatus ? (
          <span className="setting-inline-status" role="status">{inlineStatus}</span>
        ) : null}
        {inlineError ? (
          <span className="setting-inline-error" role="alert">{inlineError}</span>
        ) : null}
      </div>
    </div>
  );
}
