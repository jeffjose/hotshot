export interface Metadata {
  id: string;
  path: string;
  timestamp: string;
  width: number;
  height: number;
  format: string;
  capture_mode: string;
  display_server: string;
  file_size: number;
  tags: string[];
  notes: string;
}

export interface Monitor {
  name: string;
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface Config {
  storage_dir: string;
  image: ImageConfig;
  storage: StorageConfig;
  behavior: BehaviorConfig;
}

export interface ImageConfig {
  format: string;
  quality: number;
  filename_template: string;
}

export interface StorageConfig {
  organize_by: string;
}

export interface BehaviorConfig {
  copy_to_clipboard: boolean;
  notification: boolean;
}
