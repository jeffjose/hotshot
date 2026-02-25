import { writable } from "svelte/store";
import type { Metadata } from "../types";

export const screenshots = writable<Metadata[]>([]);
export const sidebarOpen = writable(true);
export const searchQuery = writable("");
