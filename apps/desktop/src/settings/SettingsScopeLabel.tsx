import type { SettingsScope } from "./settingsCatalog";

export function SettingsScopeLabel({ scope }: { scope: SettingsScope }) {
  return (
    <span className="settings-scope-label" aria-label={`Scope: ${scope}`}>
      {scope}
    </span>
  );
}
