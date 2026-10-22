export type LinkState = "connected" | "reconnecting" | "disconnected";
export type InternetState = "unknown" | "up" | "down";

export type InternetStatus = {
  state: InternetState;
  lastOkTs: number | null;
  latencyMs: number | null;
  linkAgentAlive: boolean | null;
};

export type CpuStatus = {
  usagePct: number | null;
  load1: number | null;
  cores: number;
};

export type MemStatus = {
  totalKb: number | null;
  availableKb: number | null;
  swapTotalKb: number | null;
  swapFreeKb: number | null;
};

export type NetStatus = {
  iface: string | null;
  rxBytesPerSec: number | null;
  txBytesPerSec: number | null;
};

export type DiskStatus = {
  mount: string;
  totalBytes: number;
  usedBytes: number;
};

export type TempStatus = {
  label: string;
  celsius: number;
};

export type PortMap = {
  host: number;
  container: number;
  proto: string;
};

export type ContainerStatus = {
  name: string;
  image: string;
  state: string;
  status: string;
  health: string | null;
  ports: PortMap[];
};

export type RaidStatus = {
  name: string;
  level: string;
  disksUp: number;
  disksTotal: number;
  devices: string;
  healthy: boolean;
  recovering: boolean;
  recoveryPct: number | null;
};

export type HealthState = "unknown" | "up" | "degraded" | "down";

export type ItemHealth = {
  state: HealthState;
  lastOkTs: number | null;
  lastCheckedTs: number | null;
  changedTs: number | null;
  latencyMs: number | null;
  reason: string | null;
};

export type EventSeverity = "warning" | "critical";

export type HealthEvent = {
  id: string;
  source: "nas" | "internet" | "service" | "container" | "raid" | "drive" | string;
  sourceId: string;
  title: string;
  severity: EventSeverity;
  startedAt: number;
  endedAt: number | null;
  message: string;
  reason: string | null;
};

export type NavItem = {
  id: string;
  title: string;
  icon: string;
  url: string;
  host: string;
  port: number;
  containerName: string | null;
  group: string;
  order: number;
  favorite: boolean;
  healthCheck: boolean;
};

export type CandidateSource = "docker" | "access";

export type Candidate = {
  host: string;
  port: number;
  suggestedTitle: string;
  url: string;
  containerName: string | null;
  source: CandidateSource;
  proto: string;
};

export type OverlayDisplay = {
  showLabels: boolean;
  precision: number;
  showLink: boolean;
  showInternet: boolean;
  showCpu: boolean;
  showCpuTemp: boolean;
  showMem: boolean;
  showMemTemp: boolean;
  showNet: boolean;
  showAccess: boolean;
  showPorts: boolean;
  snapHide: boolean;
  opaque: boolean;
};

export type OverlayConfig = {
  x: number | null;
  y: number | null;
  collapsed: boolean;
  visible: boolean;
  dock: "left" | "right" | null;
  display: OverlayDisplay;
};

export type SearchConfig = {
  everythingHttp: string;
  alistUrl: string;
  alistContainer: string;
  alistBackend: "alist" | "openlist";
  quarkMount: string;
  baiduMount: string;
  everythingExecutable: string;
};

export type CollectorConfig = {
  netIface: string | null;
  diskMounts: string[];
  probeHosts: string[];
  linkAgentProcess: string | null;
};

export type SearchHit = {
  source: "local" | "nas" | "quark" | "baidu" | string;
  name: string;
  path: string;
  isDir: boolean;
  size: number | null;
  openUrl: string | null;
};

export type SearchResponse = {
  query: string;
  hits: SearchHit[];
  warnings: string[];
};

export type DriveMount = {
  id: "quark" | "baidu" | string;
  label: string;
  mountPath: string;
  state: "unknown" | "up" | "down" | "missing" | string;
  status: string;
  message: string;
  checkedTs: number;
  latencyMs: number | null;
};

export type AppConfig = {
  schemaVersion: number;
  sshAlias: string;
  nasHost: string;
  navItems: NavItem[];
  overviewContainerNames: string[];
  watchPorts: number[];
  containerTitles: Record<string, string>;
  collector: CollectorConfig;
  ui: {
    intervalMs: number;
    internetProbeEnabled: boolean;
    autostart: boolean;
    sidebarCollapsed: boolean;
    navCompact: boolean;
    bloubPin?: string;
    overlay: OverlayConfig;
  };
  search: SearchConfig;
};

export type ActiveAccess = {
  port: number;
  navItemId: string | null;
  title: string | null;
  containerName: string | null;
  connections: number;
};

export type MetricsSnapshot = {
  ts: number;
  link: LinkState;
  lastSuccessTs: number | null;
  internet: InternetStatus;
  cpu: CpuStatus;
  mem: MemStatus;
  uptimeSec: number | null;
  net: NetStatus;
  disks: DiskStatus[];
  temps: TempStatus[];
  raids: RaidStatus[];
  containers: ContainerStatus[];
  activeAccess: ActiveAccess[];
};
