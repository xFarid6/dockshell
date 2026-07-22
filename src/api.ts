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

export interface MountInfo {
  source: string;
  destination: string;
  mode: string;
}

export interface PortBinding {
  containerPort: string;
  hostIp: string;
  hostPort: string;
}

export interface ContainerDetail {
  id: string;
  name: string;
  image: string;
  state: string;
  health: string | null;
  created: string;
  restartPolicy: string;
  env: string[];
  labels: Record<string, string>;
  mounts: MountInfo[];
  ports: PortBinding[];
}

export interface LogLine {
  stream: string;
  message: string;
}

export interface ImageInfo {
  id: string;
  tags: string[];
  size: number;
  created: number;
}

export interface PullProgress {
  id: string | null;
  status: string;
  progress: string | null;
}

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

export const inspectContainer = (connectionId: string, containerId: string) =>
  invoke<ContainerDetail>("inspect_container", { connectionId, containerId });

export const listImages = (connectionId: string) =>
  invoke<ImageInfo[]>("list_images", { connectionId });

export const removeImage = (connectionId: string, image: string, force: boolean) =>
  invoke<void>("remove_image", { connectionId, image, force });

/** Resolves once the pull completes; progress arrives as `pull-progress:{image}` events. */
export const pullImage = (connectionId: string, image: string) =>
  invoke<void>("pull_image", { connectionId, image });

export const startLogStream = (connectionId: string, containerId: string) =>
  invoke<void>("start_log_stream", { connectionId, containerId });

export const stopLogStream = (containerId: string) =>
  invoke<void>("stop_log_stream", { containerId });

export type ExecShell = "/bin/sh" | "/bin/bash";

/** Returns the exec ID: output arrives as `exec-output:{execId}` events. */
export const startExec = (
  connectionId: string,
  containerId: string,
  shell: ExecShell,
) => invoke<string>("start_exec", { connectionId, containerId, shell });

export const writeExecInput = (execId: string, data: string) =>
  invoke<void>("write_exec_input", { execId, data });

export const resizeExec = (execId: string, cols: number, rows: number) =>
  invoke<void>("resize_exec", { execId, cols, rows });

export const stopExec = (execId: string) =>
  invoke<void>("stop_exec", { execId });
