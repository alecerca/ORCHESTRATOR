// Server configuration store (persisted to localStorage)
import { useSyncExternalStore } from "react";
import type { ServerConfig } from "./types";

const STORAGE_KEY = "orquestador.serverConfig";
const DEFAULT: ServerConfig = {
  baseUrl: "http://127.0.0.1:4096",
};

type Listener = () => void;

const listeners = new Set<Listener>();
let loaded = false;
let config: ServerConfig = DEFAULT;

function load(): ServerConfig {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return DEFAULT;
    const parsed = JSON.parse(raw) as Partial<ServerConfig>;
    if (parsed && typeof parsed.baseUrl === "string" && parsed.baseUrl.trim()) {
      return {
        baseUrl: parsed.baseUrl.trim(),
        username: parsed.username,
        password: parsed.password,
      };
    }
  } catch {
    // ignore
  }
  return DEFAULT;
}

function ensureLoaded(): void {
  if (loaded) return;
  loaded = true;
  config = load();
}

function emit(): void {
  listeners.forEach((l) => l());
}

function persist(c: ServerConfig): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(c));
  } catch {
    // ignore
  }
}

export function getConfig(): ServerConfig {
  ensureLoaded();
  return config;
}

export function getSnapshot(): ServerConfig {
  ensureLoaded();
  return config;
}

export function setConfig(c: Partial<ServerConfig>): void {
  ensureLoaded();
  const next: ServerConfig = {
    baseUrl: c.baseUrl?.trim() || config.baseUrl,
    username: c.username,
    password: c.password,
  };
  config = next;
  persist(next);
  emit();
}

export function subscribe(cb: () => void): () => void {
  listeners.add(cb);
  return () => listeners.delete(cb);
}

export function useServerConfig(): ServerConfig {
  return useSyncExternalStore(subscribe, getSnapshot);
}
