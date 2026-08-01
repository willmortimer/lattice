export interface GuidanceAnchor {
  id: string;
  isAvailable(): boolean;
  reveal(): Promise<void>;
  getRect(): DOMRect | null;
  focus?(): void;
  describe?(): string;
}
