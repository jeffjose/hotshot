import { writable } from "svelte/store";

export interface Annotation {
  id: string;
  type: "rect" | "arrow" | "text" | "pen";
  x: number;
  y: number;
  props: Record<string, unknown>;
}

export const annotations = writable<Annotation[]>([]);
export const undoStack = writable<Annotation[][]>([]);
export const redoStack = writable<Annotation[][]>([]);
