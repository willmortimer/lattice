import { useSettingsHighlight } from "./settingsHighlight";
import { settingScopeForId, type SettingsScope } from "./settingsCatalog";
import { SettingsScopeLabel } from "./SettingsScopeLabel";

export function SettingRow({
  settingId,
  title,
  description,
  inlineStatus,
  inlineError,
  scope,
  children,
}: {
  settingId?: string;
  title: string;
  description: string;
  inlineStatus?: string | null;
  inlineError?: string | null;
  scope?: SettingsScope | null;
  children: React.ReactNode;
}) {
  const highlighted = useSettingsHighlight(settingId);
  const resolvedScope = scope ?? settingScopeForId(settingId);

  return (
    <div
      className={`setting-row${highlighted ? " setting-row-highlight" : ""}`}
      data-setting-id={settingId}
    >
      <div>
        <div className="setting-row-title">
          <strong>{title}</strong>
          {resolvedScope ? <SettingsScopeLabel scope={resolvedScope} /> : null}
        </div>
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
