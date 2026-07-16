export function markPlatform(): void {
  const platform = navigator.platform.toLowerCase();
  document.documentElement.dataset.platform = platform.includes("mac")
    ? "macos"
    : platform.includes("win")
      ? "windows"
      : "linux";
}
