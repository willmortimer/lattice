import { invoke } from "@tauri-apps/api/core";

import {
  demoPackageInterfaces,
  demoPackageInterfacesByPath,
} from "../demoWorkspace.generated";
import type {
  InterfaceComponent,
  InterfaceDef,
  InterfaceLayout,
  InterfaceParameter,
} from "../lib/bindingSpec";
import { interfaceHasDashboardComponents } from "../lib/bindingSpec";

/** Mirrors Tauri `InterfaceSummary` from `list_data_interfaces` / `load_data_interface`. */
export interface InterfaceSummary extends InterfaceDef {
  name: string;
  views: string[];
  forms: string[];
  title?: string;
  description?: string;
  parameters?: Record<string, InterfaceParameter>;
  layout?: InterfaceLayout;
  components?: InterfaceComponent[];
}

/**
 * Browser-demo package interfaces compiled from the First Look template seed.
 */
export const DEMO_PACKAGE_INTERFACES: InterfaceSummary[] = demoPackageInterfaces;

/** Interfaces keyed by `.data` package path for multi-app browser demos. */
export const DEMO_PACKAGE_INTERFACES_BY_PATH: Record<string, InterfaceSummary[]> =
  demoPackageInterfacesByPath ?? {};

/** Multi-component demo fixture (≥3 component types) for browser / tests. */
export const DEMO_OPS_DASHBOARD: InterfaceSummary = {
  name: "OpsDashboard",
  views: ["Board"],
  forms: ["ContactIntake"],
  title: "Ops dashboard",
  description: "Multi-component CRM interface (metric, chart, map, data-view, form).",
  parameters: {
    region: { type: "string", default: "all" },
  },
  layout: { columns: 12 },
  components: [
    {
      id: "contact_count",
      type: "metric",
      span: 3,
      title: "Contacts",
      binding: {
        type: "sqlite-query",
        resource: ".",
        sql: "SELECT COUNT(*) AS value FROM contacts",
        limit: 1,
      },
    },
    {
      id: "revenue_chart",
      type: "chart",
      span: 6,
      title: "Revenue by region",
      chart: "Dashboards/Revenue by region and category.vl.json",
      binding: {
        type: "duckdb-query",
        resources: ["Data/Orders.dataset"],
        sql: "SELECT region, sum(revenue) AS revenue FROM read_parquet('Data/Orders.dataset/facts/**/*.parquet', hive_partitioning = true, union_by_name = true) WHERE ('{{region}}' = 'all' OR region = '{{region}}') GROUP BY region ORDER BY region",
        limit: 100,
      },
    },
    {
      id: "places_map",
      type: "map",
      span: 6,
      title: "Places",
      binding: {
        type: "duckdb-query",
        resources: ["Data/Places.dataset"],
        sql: "SELECT * FROM read_parquet('Data/Places.dataset/facts/**/*.parquet', hive_partitioning = true, union_by_name = true) WHERE ('{{region}}' = 'all' OR '{{region}}' IS NOT NULL)",
        limit: 500,
      },
    },
    {
      id: "board",
      type: "data-view",
      span: 6,
      title: "Board",
      binding: { type: "saved-view", resource: ".", view: "Board" },
    },
    {
      id: "intake",
      type: "form",
      span: 6,
      form: "ContactIntake",
      binding: { type: "resource", resource: "." },
    },
  ],
};

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

export { interfaceHasDashboardComponents };
