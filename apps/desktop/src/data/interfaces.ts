import { invoke } from "@tauri-apps/api/core";

import {
  demoPackageInterfaces,
  demoPackageInterfacesByPath,
} from "../demoWorkspace.generated";

/** Mirrors Tauri `InterfaceSummary` from `list_data_interfaces` / `load_data_interface`. */
export interface InterfaceSummary {
  name: string;
  views: string[];
  forms: string[];
  title?: string;
  description?: string;
}

/**
 * Browser-demo package interfaces compiled from the First Look template seed.
 */
export const DEMO_PACKAGE_INTERFACES: InterfaceSummary[] = demoPackageInterfaces;

/** Interfaces keyed by `.data` package path for multi-app browser demos. */
export const DEMO_PACKAGE_INTERFACES_BY_PATH: Record<string, InterfaceSummary[]> =
  demoPackageInterfacesByPath ?? {};

export function demoInterfacesForPackage(relPath: string): InterfaceSummary[] {
  return DEMO_PACKAGE_INTERFACES_BY_PATH[relPath] ?? DEMO_PACKAGE_INTERFACES;
}

export async function listDataInterfaces(root: string, relPath: string): Promise<string[]> {
  return invoke<string[]>("list_data_interfaces", { root, relPath });
}

export async function loadDataInterface(
  root: string,
  relPath: string,
  name: string,
): Promise<InterfaceSummary> {
  return invoke<InterfaceSummary>("load_data_interface", { root, relPath, name });
}

/** List interfaces for native IPC or return demo fixtures in browser mode. */
export async function listPackageInterfaces(options: {
  root: string;
  relPath: string;
  demo?: boolean;
  demoInterfaces?: InterfaceSummary[];
}): Promise<string[]> {
  if (options.demo) {
    return (options.demoInterfaces ?? demoInterfacesForPackage(options.relPath)).map(
      (iface) => iface.name,
    );
  }
  return listDataInterfaces(options.root, options.relPath);
}

/** Load one interface for native IPC or resolve from demo fixtures. */
export async function loadPackageInterface(options: {
  root: string;
  relPath: string;
  name: string;
  demo?: boolean;
  demoInterfaces?: InterfaceSummary[];
}): Promise<InterfaceSummary> {
  if (options.demo) {
    const interfaces =
      options.demoInterfaces ?? demoInterfacesForPackage(options.relPath);
    const match = interfaces.find((iface) => iface.name === options.name);
    if (!match) {
      throw new Error(`Unknown demo interface: ${options.name}`);
    }
    return match;
  }
  return loadDataInterface(options.root, options.relPath, options.name);
}
