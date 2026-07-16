// Typed wrappers over the Tauri IPC surface (src-tauri/src/commands.rs).

import { invoke } from "@tauri-apps/api/core";

export interface ConnectionInfo {
  id: string;
  name: string;
  /** "local" for the platform socket, or tcp://host:port for remote. */
  endpoint: string;
  useTls: boolean;
}

export interface ContainerInfo {
  id: string;
  name: string;
  image: string;
  state: string;
  status: string;
  ports: string[];
}

export type ContainerAction = "start" | "stop" | "restart";

export const listConnections = () =>
  invoke<ConnectionInfo[]>("list_connections");

export const saveConnection = (info: ConnectionInfo, secret?: string) =>
  invoke<void>("save_connection", { info, secret: secret ?? null });

export const deleteConnection = (id: string) =>
  invoke<void>("delete_connection", { id });

export const testConnection = (id: string) =>
  invoke<string>("test_connection", { id });

export const listContainers = (connectionId: string) =>
  invoke<ContainerInfo[]>("list_containers", { connectionId });

export const containerAction = (
  connectionId: string,
  containerId: string,
  action: ContainerAction,
) => invoke<void>("container_action", { connectionId, containerId, action });
