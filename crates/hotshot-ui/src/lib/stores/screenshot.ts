import { writable } from "svelte/store";
import type { Metadata } from "../types";

export const currentScreenshot = writable<Metadata | null>(null);
export const currentImageSrc = writable<string>("");
export const isLoading = writable(false);
