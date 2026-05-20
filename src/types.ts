export interface ServerConfig {
  path: string;
  port: number;
  interfaces: string;
  auth_username: string;
  auth_password: string;
  upload: boolean;
  mkdir: boolean;
  color_scheme: string;
  title: string;
  compress: string;
  hidden: boolean;
  random_route: boolean;
  readme: boolean;
  download: boolean;
  webdav: boolean;
}

export interface ServerStatus {
  running: boolean;
  pid: number | null;
  url: string | null;
  urls: string[];
  port: number | null;
}

export interface EngineStatus {
  exists: boolean;
  version: string | null;
  path: string;
}

export interface QrResponse {
  data: string;
}
