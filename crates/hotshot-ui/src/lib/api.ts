import { invoke } from "@tauri-apps/api/core";
import type { Metadata, Monitor, Config } from "./types";

export async function captureFullscreen(
  display?: string,
  copyToClipboard?: boolean,
): Promise<Metadata> {
  return invoke("capture_fullscreen", {
    display,
    copyToClipboard: copyToClipboard ?? true,
  });
}

export async function captureRegion(
  display?: string,
  copyToClipboard?: boolean,
): Promise<Metadata> {
  return invoke("capture_region", {
    display,
    copyToClipboard: copyToClipboard ?? true,
  });
}

export async function captureWindow(
  copyToClipboard?: boolean,
): Promise<Metadata> {
  return invoke("capture_window", {
    copyToClipboard: copyToClipboard ?? true,
  });
}

export async function listScreenshots(
  limit?: number,
): Promise<Metadata[]> {
  return invoke("list_screenshots", { limit });
}

export async function getScreenshot(id: string): Promise<Metadata> {
  return invoke("get_screenshot", { id });
}

export async function searchScreenshots(
  query: string,
): Promise<Metadata[]> {
  return invoke("search_screenshots", { query });
}

export async function deleteScreenshot(id: string): Promise<Metadata> {
  return invoke("delete_screenshot", { id });
}

export async function tagScreenshot(
  id: string,
  tags: string[],
): Promise<Metadata> {
  return invoke("tag_screenshot", { id, tags });
}

export async function readScreenshotImage(id: string): Promise<string> {
  return invoke("read_screenshot_image", { id });
}

export async function listMonitors(): Promise<Monitor[]> {
  return invoke("list_monitors");
}

export async function getConfig(): Promise<Config> {
  return invoke("get_config");
}

export async function updateConfig(
  key: string,
  value: string,
): Promise<Config> {
  return invoke("update_config", { key, value });
}

export function imageUrl(path: string): string {
  return `hotshot://localhost/${path}`;
}
