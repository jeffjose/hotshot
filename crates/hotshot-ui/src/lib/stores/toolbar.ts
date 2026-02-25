import { writable } from "svelte/store";

export type Tool = "select" | "rect" | "arrow" | "text" | "pen";

export const activeTool = writable<Tool>("select");
export const strokeColor = writable("#ff0000");
export const strokeWidth = writable(3);
